use bytes::Bytes;
use clap::{CommandFactory, FromArgMatches, Parser};
use log::{debug, error, info};
use mb_tool::error::DynResult;
use mb_tool::modbus_connection::{self, ModbusOptions};
use mb_tool::observable_array::ObservableArray;
use mb_tool::tag_list_xml;
use mb_tool::tag_ranges::TagRanges;
use mb_tool::tags::Tags;
use mb_tool::template;
use mb_tool::web_server;
use roxmltree::Document;
use rust_embed::RustEmbed;
use serde_derive::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::net::IpAddr;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, RwLock};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tokio_modbus::slave::Slave;
use tokio_serial::{Parity,SerialStream};

#[derive(Serialize, Deserialize)]
enum MbCommands {
    UpdateHoldingRegs { start: u16, regs: Vec<u16> },
    UpdateInputRegs { start: u16, regs: Vec<u16> },
    UpdateDiscreteInputs { start: u16, regs: Vec<bool> },
    UpdateCoils { start: u16, regs: Vec<bool> },
    RequestHoldingRegs { start: u16, length: u16 },
    RequestInputRegs { start: u16, length: u16 },
    RequestDiscreteInputs { start: u16, length: u16 },
    RequestCoils { start: u16, length: u16 },
    Echo(i64),
}

fn ws_request<T, F>(
    array: &ObservableArray<T>,
    mb_send: &broadcast::Sender<Bytes>,
    start: u16,
    length: u16,
    f: F,
) where
    F: FnOnce(u16, Vec<T>) -> MbCommands,
    T: Default + Clone,
{
    let reply = array.get_array(|r| {
        f(
            start,
            Vec::from(&r[(start as usize)..(start as usize + length as usize)]),
        )
    });

    let bytes = Bytes::from(serde_json::to_string(&reply).unwrap());
    // Only returns error if no client is connected
    let _ = mb_send.send(bytes);
}

async fn mb_task(
    tags: Tags,
    mb_send: broadcast::Sender<Bytes>,
    mut mb_receive: mpsc::Receiver<Bytes>,
) {
    loop {
        tokio::select! {
            Some(data) = mb_receive.recv() =>
            {
                match std::str::from_utf8(&data[..]) {

                    Ok(json) => {
                        debug!("JSON: {}", json);
                        match serde_json::from_str::<MbCommands>(json) {
                Ok(cmd) => {
                match cmd {
                    // Holding registers
                                    MbCommands::RequestHoldingRegs{start, length} => {
                    debug!("RequestHoldingRegs");
                    ws_request(&tags.holding_registers, &mb_send,start, length,
                           |start, regs| {
                               MbCommands::UpdateHoldingRegs{start, regs}
                           });

                                    } ,
                                    MbCommands::UpdateHoldingRegs{start, regs: reg_data} => {
                    tags.holding_registers.update(start as usize, &reg_data);
                                    } ,

                    // Input registers
                    MbCommands::RequestInputRegs{start, length} => {
                    debug!("RequestInputRegs");
                    ws_request(&tags.input_registers, &mb_send,start, length,
                           |start, regs| {
                               MbCommands::UpdateInputRegs{start, regs}
                           });
                                    } ,
                                    MbCommands::UpdateInputRegs{start, regs: reg_data} => {
                    tags.input_registers.update(start as usize, &reg_data);
                                    } ,

                    // Coils
                    MbCommands::RequestCoils{start, length} => {
                    debug!("RequestCoils");
                    ws_request(&tags.coils, &mb_send,start, length,
                           |start, regs| {
                               MbCommands::UpdateCoils{start, regs}
                           });
                                    } ,
                                    MbCommands::UpdateCoils{start, regs: reg_data} => {
                    tags.coils.update(start as usize, &reg_data);
                                    } ,

                    // Discrete inputs
                    MbCommands::RequestDiscreteInputs{start, length} => {
                    debug!("RequestDiscreteInputs");
                    ws_request(&tags.discrete_inputs, &mb_send,start, length,
                           |start, regs| {
                               MbCommands::UpdateDiscreteInputs{start, regs}
                           });
                                    } ,
                                    MbCommands::UpdateDiscreteInputs{start, regs: reg_data} => {
                    tags.discrete_inputs.update(start as usize, &reg_data);
                                    } ,

                    MbCommands::Echo(count) => {
                        let reply = MbCommands::Echo(count);
                        let bytes = Bytes::from(serde_json::to_string(&reply).unwrap());
                        let _ = mb_send.send(bytes);
                    }
                }
                }
                Err(e) => {
                error!("Failed to parse JSON message: {e}");
                }

            }
                    }
                    Err(e) => error!("Illegal UTF-8 in message from client: {}", e)

                }
            },
            updated = tags.holding_registers.updated() =>
            {
                for range in &updated {
                    let cmd =
                        tags.holding_registers.get_array(|r| {
                            MbCommands::UpdateHoldingRegs{start:range.start as u16, regs: Vec::from(&r[range.start .. range.end])}
                        });

                    let bytes = Bytes::from(serde_json::to_string(&cmd).unwrap());
                    let _ = mb_send.send(bytes);
                }
            },
            updated = tags.input_registers.updated() =>
            {
                for range in &updated {
                    let cmd =
                        tags.input_registers.get_array(|r| {
                            MbCommands::UpdateInputRegs{start:range.start as u16, regs: Vec::from(&r[range.start .. range.end])}
                        });

                    let bytes = Bytes::from(serde_json::to_string(&cmd).unwrap());
                    let _ = mb_send.send(bytes);
                }
            }
        updated = tags.coils.updated() =>
            {
                for range in &updated {
                    let cmd =
                        tags.coils.get_array(|r| {
                            MbCommands::UpdateCoils{start:range.start as u16, regs: Vec::from(&r[range.start .. range.end])}
                        });

                    let bytes = Bytes::from(serde_json::to_string(&cmd).unwrap());
                    let _ = mb_send.send(bytes);
                }
            }
        updated = tags.discrete_inputs.updated() =>
            {
                for range in &updated {
                    let cmd =
                        tags.discrete_inputs.get_array(|r| {
                            MbCommands::UpdateDiscreteInputs{start:range.start as u16, regs: Vec::from(&r[range.start .. range.end])}
                        });

                    let bytes = Bytes::from(serde_json::to_string(&cmd).unwrap());
                    let _ = mb_send.send(bytes);
                }
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
    let tag_list = tag_list_xml::parse_tag_list(&top).unwrap();

    let tag_list = Arc::new(RwLock::new(tag_list));
    let (ws_send, mb_receive) = mpsc::channel(4);
    let (mb_send, _) = broadcast::channel(4);
    let tags = Tags::new(&tag_list.read().unwrap());
    let ranges = TagRanges::from(&*tag_list.read().unwrap());
    debug!("Register ranges: {:?}", ranges);
    tokio::spawn(mb_task(tags.clone(), mb_send.clone(), mb_receive));
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
                Some(_) | None => Parity::Even
            };
            let builder = tokio_serial::new(&path, args.baud_rate.unwrap_or(9600)).parity(parity);
            match SerialStream::open(&builder) {
                Ok(ser) => {
                    join = tokio::spawn(modbus_connection::server_rtu(
                        ser,
                        tags.clone(),
                        ranges,
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
                tags.clone(),
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
                Some(_) | None => Parity::Even
            };
            let builder = tokio_serial::new(&path, args.baud_rate.unwrap_or(9600)).parity(parity);
            match SerialStream::open(&builder) {
                Ok(ser) => {
                    join = tokio::spawn(modbus_connection::client_rtu(
                        ser,
                        Slave(args.mb_address),
                        tags.clone(),
                        ranges,
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
                Slave(args.mb_address),
                tags.clone(),
                ranges,
                mb_options,
            ));
            info!("Running as TCP client connected to {}:{}", addr, port);
        }
    }

    let mut conf = web_server::ServerConfig::new(ws_send, mb_send);

    if let Some(bind) = args.http_address {
        conf = conf.bind_addr(IpAddr::V4(bind));
    }
    conf = conf.port(args.http_port);
    let conf = conf.build_page(match template::build_page::<WebTemplates>(tag_list) {
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

    let (server, bound_ip, bound_port) = web_server::setup_server(conf);

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
