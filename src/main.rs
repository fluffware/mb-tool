use bytes::Bytes;
use clap::{CommandFactory, FromArgMatches, Parser};
use log::{debug, error, info};
use mb_tool::device_list_xml;
use mb_tool::devices::Devices;
use mb_tool::error::DynResult;
use mb_tool::modbus_connection::{self, ModbusOptions};
use mb_tool::observable_array::ObservableArray;
use mb_tool::tags::{Tags, Updated};
use mb_tool::template;
use mb_tool::web_server;
use mb_tool::web_server::{WebsocketConnect, WebsocketReceive, WsSender};
use roxmltree::Document;
use rust_embed::RustEmbed;
use serde_derive::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::net::IpAddr;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tokio_modbus::Slave;
use tokio_serial::{Parity, SerialStream};

#[derive(Serialize, Deserialize)]
enum MbCommands {
    UpdateHoldingRegs {
        unit_addr: u8,
        start: u16,
        regs: Vec<u16>,
    },
    UpdateInputRegs {
        unit_addr: u8,
        start: u16,
        regs: Vec<u16>,
    },
    UpdateDiscreteInputs {
        unit_addr: u8,
        start: u16,
        regs: Vec<bool>,
    },
    UpdateCoils {
        unit_addr: u8,
        start: u16,
        regs: Vec<bool>,
    },
    RequestHoldingRegs {
        unit_addr: u8,
        start: u16,
        length: u16,
    },
    RequestInputRegs {
        unit_addr: u8,
        start: u16,
        length: u16,
    },
    RequestDiscreteInputs {
        unit_addr: u8,
        start: u16,
        length: u16,
    },
    RequestCoils {
        unit_addr: u8,
        start: u16,
        length: u16,
    },
    ListUnitAddresses(Vec<u8>),
    Echo(i64),
}

struct WsHandler {
    devices: Devices,
}

impl WsHandler {
    fn new(devices: Devices) -> WsHandler {
        WsHandler { devices }
    }
}

impl WebsocketConnect for WsHandler {
    fn connected(&self, send: WsSender) -> Box<dyn WebsocketReceive + Send + Sync> {
        debug!("Socket connected");
        let devices = self.devices.clone();
        let update_send = send.clone();
        tokio::spawn(async move {
            loop {
                let (unit_addr, updated) = devices.updated().await;
		if update_send.is_closed() {break;}
                devices
                    .tags_read(unit_addr, |tags| {
                        handle_updates(unit_addr, tags, &updated, &update_send)
                    })
                    .unwrap();
            }
        });

        Box::new(WsReceive {
            devices: self.devices.clone(),
            send,
        })
    }
}

struct WsReceive {
    devices: Devices,
    send: WsSender,
}
impl WebsocketReceive for WsReceive {
    fn message(&mut self, msg: &str) -> Option<String> {
        debug!("Received from WS: {msg}");
        handle_receive(&self.devices, &self.send, msg);
        None
    }
    fn disconnected(&mut self) {
        debug!("Disconnected from WS");
    }
}

fn ws_request<T, F>(
    array: &ObservableArray<T>,
    mb_send: &mpsc::Sender<String>,
    start: u16,
    length: u16,
    f: F,
) where
    F: FnOnce(u16, Vec<T>) -> MbCommands,
    T: Default + Clone + Send + Sync + 'static,
{
    let reply = array.get_array(|r| {
        f(
            start,
            Vec::from(&r[(start as usize)..(start as usize + length as usize)]),
        )
    });

    let _ = mb_send.send(serde_json::to_string(&reply).unwrap());
}

fn handle_receive(devices: &Devices, mb_send: &mpsc::Sender<String>, json: &str) {
    debug!("JSON: {}", json);
    match serde_json::from_str::<MbCommands>(json) {
        Ok(cmd) => {
            match cmd {
                // Holding registers
                MbCommands::RequestHoldingRegs {
                    unit_addr,
                    start,
                    length,
                } => {
                    debug!("RequestHoldingRegs");
                    devices
                        .tags_read(unit_addr, |tags| {
                            ws_request(
                                &tags.holding_registers,
                                mb_send,
                                start,
                                length,
                                |start, regs| MbCommands::UpdateHoldingRegs {
                                    unit_addr,
                                    start,
                                    regs,
                                },
                            );
                        })
                        .unwrap();
                }
                MbCommands::UpdateHoldingRegs {
                    unit_addr,
                    start,
                    regs: reg_data,
                } => {
                    debug!("UpdateHoldingRegs");
                    devices
                        .tags_write(unit_addr, |tags| {
                            tags.holding_registers.update(start as usize, &reg_data);
                        })
                        .unwrap();
                }

                // Input registers
                MbCommands::RequestInputRegs {
                    unit_addr,
                    start,
                    length,
                } => {
                    debug!("RequestInputRegs");
                    devices
                        .tags_read(unit_addr, |tags| {
                            ws_request(
                                &tags.input_registers,
                                &mb_send,
                                start,
                                length,
                                |start, regs| MbCommands::UpdateInputRegs {
                                    unit_addr,
                                    start,
                                    regs,
                                },
                            );
                        })
                        .unwrap();
                }
                MbCommands::UpdateInputRegs {
                    unit_addr,
                    start,
                    regs: reg_data,
                } => {
                    devices
                        .tags_write(unit_addr, |tags| {
                            tags.input_registers.update(start as usize, &reg_data);
                        })
                        .unwrap();
                }

                // Coils
                MbCommands::RequestCoils {
                    unit_addr,
                    start,
                    length,
                } => {
                    debug!("RequestCoils");
                    devices
                        .tags_read(unit_addr, |tags| {
                            ws_request(&tags.coils, &mb_send, start, length, |start, regs| {
                                MbCommands::UpdateCoils {
                                    unit_addr,
                                    start,
                                    regs,
                                }
                            });
                        })
                        .unwrap();
                }
                MbCommands::UpdateCoils {
                    unit_addr,
                    start,
                    regs: reg_data,
                } => {
                    devices
                        .tags_write(unit_addr, |tags| {
                            tags.coils.update(start as usize, &reg_data);
                        })
                        .unwrap();
                }

                // Discrete inputs
                MbCommands::RequestDiscreteInputs {
                    unit_addr,
                    start,
                    length,
                } => {
                    debug!("RequestDiscreteInputs");
                    devices
                        .tags_read(unit_addr, |tags| {
                            ws_request(
                                &tags.discrete_inputs,
                                &mb_send,
                                start,
                                length,
                                |start, regs| MbCommands::UpdateDiscreteInputs {
                                    unit_addr,
                                    start,
                                    regs,
                                },
                            );
                        })
                        .unwrap();
                }
                MbCommands::UpdateDiscreteInputs {
                    unit_addr,
                    start,
                    regs: reg_data,
                } => {
                    devices
                        .tags_write(unit_addr, |tags| {
                            tags.discrete_inputs.update(start as usize, &reg_data);
                        })
                        .unwrap();
                }

                MbCommands::Echo(count) => {
                    let reply = MbCommands::Echo(count);
                    let _ = mb_send.send(serde_json::to_string(&reply).unwrap());
                }
                MbCommands::ListUnitAddresses(_) => {
                    let units = devices.units().collect();
                    let reply = MbCommands::ListUnitAddresses(units);
                    let _ = mb_send.send(serde_json::to_string(&reply).unwrap());
                }
            }
        }
        Err(e) => {
            error!("Failed to parse JSON message: {e}");
        }
    }
}

fn handle_updates(unit_addr: u8, tags: &Tags, updated: &Updated, mb_send: &mpsc::Sender<String>) {
    use Updated::*;
    match updated {
        HoldingRegisters(ranges) => {
            for range in ranges {
                let cmd = tags
                    .holding_registers
                    .get_array(|r| MbCommands::UpdateHoldingRegs {
                        unit_addr,
                        start: range.start as u16,
                        regs: Vec::from(&r[range.start..range.end]),
                    });

                let _ = mb_send.send(serde_json::to_string(&cmd).unwrap());
            }
        }
        InputRegisters(ranges) => {
            for range in ranges {
                let cmd = tags
                    .input_registers
                    .get_array(|r| MbCommands::UpdateInputRegs {
                        unit_addr,
                        start: range.start as u16,
                        regs: Vec::from(&r[range.start..range.end]),
                    });

                let _ = mb_send.send(serde_json::to_string(&cmd).unwrap());
            }
        }
        Coils(ranges) => {
            for range in ranges {
                let cmd = tags.coils.get_array(|r| MbCommands::UpdateCoils {
                    unit_addr,
                    start: range.start as u16,
                    regs: Vec::from(&r[range.start..range.end]),
                });

                let _ = mb_send.send(serde_json::to_string(&cmd).unwrap());
            }
        }
        DiscreteInputs(ranges) => {
            for range in ranges {
                let cmd = tags
                    .discrete_inputs
                    .get_array(|r| MbCommands::UpdateDiscreteInputs {
                        unit_addr,
                        start: range.start as u16,
                        regs: Vec::from(&r[range.start..range.end]),
                    });

                let _ = mb_send.send(serde_json::to_string(&cmd).unwrap());
            }
        }
    }
}

#[derive(Parser, Debug)]
struct CmdArgs {
    /// Tag list configuration
    tag_list_conf: PathBuf,
    /// Run as server
    #[arg(long, default_value_t = false)]
    server: bool,
    /// IP-address of server
    #[arg(long)]
    ip_address: Option<Ipv4Addr>,
    /// Modbus address of server
    #[arg(long, default_value_t = 1)]
    mb_address: u8,
    /// Modbus TCP port
    #[arg(long, default_value_t = 502)]
    ip_port: u16,
    /// Serial device
    #[arg(long)]
    serial_device: Option<String>,
    /// Baud rate of serial port
    #[arg(long)]
    baud_rate: Option<u32>,
    // Parity of serial port
    #[arg(long, default_value = "Even")]
    parity: String,
    /// Bind HTTP-server to this address
    #[arg(long)]
    http_address: Option<Ipv4Addr>,
    /// HTTP port
    #[arg(long, default_value_t = 0)]
    http_port: u16,
    /// Time in milliseconds between client polls
    #[arg(long, default_value_t = 100)]
    poll_interval: u64,
}

#[cfg(feature = "webbrowser")]
mod browser {
    use clap::{Arg, ArgMatches, Command};
    use futures_util::future::FutureExt;
    use log::{info, warn};
    use std::future::Future;
    use std::pin::Pin;
    use tokio::time::sleep;
    use tokio::time::Duration;

    pub fn add_args(cmd: Command) -> Command {
        cmd.arg(
            Arg::new("start_browser")
                .value_name("START")
                .long("start-browser")
                .default_value("true")
                .value_parser(clap::value_parser!(bool))
                .help("Don't try to start a web browser"),
        )
    }

    pub fn start(matches: &ArgMatches, url: &str) -> impl Future {
        let start_browser = *matches.get_one::<bool>("start_browser").unwrap();
        let url = url.to_owned();
        let browser_start: Pin<Box<dyn Future<Output = ()>>> = if start_browser {
            let timeout = sleep(Duration::from_secs(2));
            Box::pin(
                timeout
                    .then(|_| async move {
                        info!("Starting browser for {url}");
                        match webbrowser::open(&url) {
                            Err(e) => warn!("Failed to open control page in browser: {e}"),
                            _ => {}
                        }
                    })
                    .then(|_| std::future::pending()),
            )
        } else {
            info!("Connect a web browser to {url}");
            Box::pin(std::future::pending())
        };
        browser_start
    }
}

#[cfg(not(feature = "webbrowser"))]
mod browser {
    use clap::{ArgMatches, Command};
    use log::info;
    use std::future::Future;

    pub fn add_args(cmd: Command) -> Command {
        cmd
    }

    pub fn start(_matches: &ArgMatches, url: &str) -> impl Future {
        info!("Connect a web browser to {url}");
        Box::pin(std::future::pending::<()>())
    }
}

#[derive(RustEmbed)]
#[folder = "web"]
#[include = "*.html"]
#[include = "*.js"]
#[include = "*.css"]
#[include = "*.svg"]
struct WebFiles;

#[derive(RustEmbed)]
#[folder = "web/templates"]
struct WebTemplates;

#[tokio::main]
pub async fn main() -> ExitCode {
    tracing_subscriber::fmt::init();

    let cmd = CmdArgs::command();

    let cmd = browser::add_args(cmd);

    let matches = cmd.get_matches();
    let args = match CmdArgs::from_arg_matches(&matches) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    let mut f = match File::open(&args.tag_list_conf) {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to open '{}': {}", args.tag_list_conf.display(), e);
            return ExitCode::FAILURE;
        }
    };
    let mut xml = String::new();
    f.read_to_string(&mut xml).unwrap();
    let doc = Document::parse(&xml).unwrap();
    let top = doc.root_element();
    let device_list = match device_list_xml::parse_device_list(&top) {
        Ok(l) => l,
        Err(e) => {
            error!("Failed to parse '{}': {}", args.tag_list_conf.display(), e);
            return ExitCode::FAILURE;
        }
    };
    let device_list = Arc::new(device_list);

    let devices = Devices::new(&device_list);

    //let (mb_send, _) = broadcast::channel(4);

    //tokio::spawn(mb_task(devices.clone(), mb_send.clone(), mb_receive));
    let mb_options = ModbusOptions {
        poll_interval: Duration::from_millis(args.poll_interval),
    };
    let join: JoinHandle<DynResult<()>>;
    if args.server {
        if let Some(path) = args.serial_device {
            let parity = match args.parity.get(..1) {
                Some("e") | Some("E") => Parity::Even,
                Some("o") | Some("O") => Parity::Odd,
                Some("n") | Some("N") => Parity::None,
                Some(_) | None => Parity::Even,
            };
            let builder = tokio_serial::new(&path, args.baud_rate.unwrap_or(9600)).parity(parity);
            match SerialStream::open(&builder) {
                Ok(ser) => {
                    join = tokio::spawn(modbus_connection::server_rtu(
                        ser,
                        devices.clone(),
                        mb_options,
                    ));

                    info!("Running as RTU server on {}", path);
                }
                Err(e) => {
                    error!("Failed to open serial port: {}", e);
                    for p in tokio_serial::available_ports().unwrap() {
                        info!("Available device {}: {:?}", p.port_name, p.port_type);
                    }
                    return ExitCode::FAILURE;
                }
            }
        } else {
            let addr = args
                .ip_address
                .unwrap_or_else(|| Ipv4Addr::new(127, 0, 0, 1));
            let port = args.ip_port;

            join = tokio::spawn(modbus_connection::server_tcp(
                SocketAddr::V4(SocketAddrV4::new(addr, port)),
                devices.clone(),
                mb_options,
            ));

            info!("Running as TCP server at {}:{}", addr, port);
        }
    } else {
        if let Some(path) = args.serial_device {
            let parity = match args.parity.get(..1) {
                Some("e") | Some("E") => Parity::Even,
                Some("o") | Some("O") => Parity::Odd,
                Some("n") | Some("N") => Parity::None,
                Some(_) | None => Parity::Even,
            };
            let builder = tokio_serial::new(&path, args.baud_rate.unwrap_or(9600)).parity(parity);
            match SerialStream::open(&builder) {
                Ok(ser) => {
                    join = tokio::spawn(modbus_connection::client_rtu(
                        ser,
                        Slave(args.mb_address),
                        devices.clone(),
                        mb_options,
                    ));
                    info!("Running as RTU client on {}", path);
                }
                Err(e) => {
                    error!("Failed to open serial port: {}", e);
                    for p in tokio_serial::available_ports().unwrap() {
                        info!("Available device {}: {:?}", p.port_name, p.port_type);
                    }
                    return ExitCode::FAILURE;
                }
            }
        } else {
            let addr = args
                .ip_address
                .unwrap_or_else(|| Ipv4Addr::new(127, 0, 0, 1));
            let port = args.ip_port;
            join = tokio::spawn(modbus_connection::client_tcp(
                SocketAddr::V4(SocketAddrV4::new(addr, port)),
                devices.clone(),
                mb_options,
            ));
            info!("Running as TCP client connected to {}:{}", addr, port);
        }
    }

    let mut conf = web_server::ServerConfig::new(Box::new(WsHandler::new(devices.clone())));

    if let Some(bind) = args.http_address {
        conf = conf.bind_addr(IpAddr::V4(bind));
    }
    conf = conf.port(args.http_port);
    let conf = conf.build_page(match template::build_page::<WebTemplates>(device_list) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to initilize web page builder: {e}");
            return ExitCode::FAILURE;
        }
    });

    let conf = conf.web_resource(Box::new(|path| {
        let mut path = path.trim_start_matches('/');
        if path.is_empty() {
            path = "index.html";
        }
        let suffix = path.rsplit('.').next().unwrap_or("");
        let mime_type = match suffix {
            "html" => "text/html",
            "hbs" => "text/x.handlebars",
            "js" => "text/javascript",
            "svg" => "image/svg+xml",
            "css" => "text/css",
            _ => "application/octet-stream",
        };
        match WebFiles::get(path) {
            Some(embedded) => Ok((mime_type, Bytes::from(embedded.data.into_owned()))),
            None => Err("Not found".into()),
        }
    }));

    let (server, bound_ip, bound_port) = match web_server::setup_server(conf) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to set up web server: {e}");
            return ExitCode::FAILURE;
        }
    };
    let url = format!("http://{}:{}", bound_ip, bound_port);
    let browser_start = browser::start(&matches, &url);

    tokio::select! {
        res = server => {
            if let Err(e) = res {
                error!("server error: {e}");
                return ExitCode::FAILURE;
    }
        },

        res = join => {
            match res {
                Ok(res) => {
                    if let Err(e) = res {
                        error!("Modbus failed: {e}");
                        return ExitCode::FAILURE;
                    }
                },
                Err(e) => {
                    error!("Modbus thread failed: {e}");
                    return ExitCode::FAILURE;
                }
            }
    }
        _ = browser_start => {}
    }
    ExitCode::SUCCESS
}
