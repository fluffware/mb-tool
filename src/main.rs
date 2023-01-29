use bytes::Bytes;
use clap::Parser;
use log::{debug, error, info};
use mb_tool::modbus_connection;
use mb_tool::observable_array::ObservableArray;
use mb_tool::tag_list_xml;
use mb_tool::tag_ranges::TagRanges;
use mb_tool::tags::Tags;
use roxmltree::Document;
use std::fs::File;
use std::io::Read;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, RwLock};
use tokio::sync::{broadcast, mpsc};
use tokio_modbus::slave::Slave;
use tokio_serial::SerialStream;

use mb_tool::build_main_page;
use mb_tool::web_server;
use serde_derive::{Deserialize, Serialize};

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
            Vec::from(&r[(start as usize)..((start + length) as usize)]),
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
                        if let Ok(cmd) = serde_json::from_str::<MbCommands>(json) {
			    
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
    mb_port: u16,
    /// Serial device
    #[arg(long)]
    serial_device: Option<String>,
    #[arg(long)]
    baud_rate: Option<u32>,
    /// HTTP port
    #[arg(long, default_value_t = 8090)]
    http_port: u16,
}

#[tokio::main]
pub async fn main() -> ExitCode {
    tracing_subscriber::fmt::init();

    let args = CmdArgs::parse();

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
    tokio::spawn(mb_task(tags.clone(), mb_send.clone(), mb_receive));
    if args.server {
        if let Some(path) = args.serial_device {
            let builder = tokio_serial::new(&path, args.baud_rate.unwrap_or(9600));
            match SerialStream::open(&builder) {
                Ok(ser) => {
                    tokio::spawn(modbus_connection::server_rtu(ser, tags.clone(), ranges));
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
            let addr = Ipv4Addr::new(127, 0, 0, 1);
            let port = 502;
            tokio::spawn(modbus_connection::server_tcp(
                SocketAddr::V4(SocketAddrV4::new(addr, port)),
                tags.clone(),
            ));
            info!("Running as TCP server at {}:{}", addr, port);
        }
    } else {
        if let Some(path) = args.serial_device {
            let builder = tokio_serial::new(&path, args.baud_rate.unwrap_or(9600));
            match SerialStream::open(&builder) {
                Ok(ser) => {
                    tokio::spawn(modbus_connection::client_rtu(
                        ser,
                        Slave(args.mb_address),
                        tags.clone(),
                        ranges,
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
        }
    }
    let conf = web_server::ServerConfig::new(ws_send, mb_send);
    let conf = conf.port(args.http_port);
    let conf = conf.build_page(build_main_page::build_page(tag_list));
    web_server::run_server(conf).await;
    ExitCode::SUCCESS
}
