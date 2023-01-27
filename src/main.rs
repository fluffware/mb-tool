use bytes::Bytes;
use clap::Parser;
use log::{debug, error, info};
use mb_tool::modbus_connection;
use mb_tool::observable_array::ObservableArray;
use mb_tool::tag_list_xml;
use mb_tool::tags::Tags;
use roxmltree::Document;
use std::fs::File;
use std::io::Read;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, RwLock};
use tokio::sync::{broadcast, mpsc};

use mb_tool::build_main_page;
use mb_tool::web_server;
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
enum MbCommands {
    UpdateHoldingRegs { start: u16, regs: Vec<u16> },
    UpdateInputRegs { start: u16, regs: Vec<u16> },
    UpdateInputs { start: u16, regs: Vec<bool> },
    UpdateCoils { start: u16, regs: Vec<bool> },
    RequestHoldingRegs { start: u16, length: u16 },
    RequestInputRegs { start: u16, length: u16 },
    RequestInputs { start: u16, length: u16 },
    RequestCoils { start: u16, length: u16 },
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
                                MbCommands::RequestHoldingRegs{start, length} => {
                                    debug!("RequestHoldingRegs");
                                    let reply =
                                    tags.holding_registers.get_array(|r| {
                                        MbCommands::UpdateHoldingRegs{start:0, regs: Vec::from(&r[(start as usize) .. ((start+length) as usize)])}

                                    });
                                    let bytes = Bytes::from(serde_json::to_string(&reply).unwrap());
                                    // Only returns error if no client is connected
                                    let _ = mb_send.send(bytes);
                                } ,
                                MbCommands::UpdateHoldingRegs{start, regs: reg_data} => {
                                    tags.holding_registers.update(start as usize, &reg_data);
                                    debug!("updated: {}", start)
                                } ,
                                _cmd => {
                                    error!("Unhandled command")
                                }
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
    /// Address of server
    #[arg()]
    address: Option<Ipv4Addr>,
    #[arg(long, default_value_t = 502)]
    /// TCP port
    port: u16,
    /// Serial device
    #[arg(long)]
    serial_device: Option<PathBuf>,
    #[arg(long)]
    baud_rate: Option<u32>,
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
    tokio::spawn(mb_task(tags.clone(), mb_send.clone(), mb_receive));
    if let Some(path) = args.serial_device {
        for p in tokio_serial::available_ports().unwrap() {
            info!("{}: {:?}", p.port_name, p.port_type);
        }
        tokio::spawn(modbus_connection::server_rtu(path, args.baud_rate.unwrap_or(9600),tags.clone()));
    } else {
        tokio::spawn(modbus_connection::server_tcp(
            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 502)),
            tags.clone(),
        ));
    }
    let conf = web_server::ServerConfig::new(ws_send, mb_send);
    let conf = conf.build_page(build_main_page::build_page(tag_list));
    web_server::run_server(conf).await;
    ExitCode::SUCCESS
}
