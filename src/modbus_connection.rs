use crate::error::DynResult;
use crate::observable_array::ObservableArray;
use crate::tags::Tags;
use bytes::Bytes;
use std::future::{self, Future};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::pin::Pin;
use tokio_modbus::server::tcp::Server as TcpServer;
use tokio_modbus::server::rtu::Server as RtuServer;
use std::path::Path;

#[allow(unused_imports)]
use log::{debug, error};

fn err_resp(req: tokio_modbus::prelude::Request, exception: u8) -> tokio_modbus::prelude::Response {
    tokio_modbus::prelude::Response::Custom(0x80 | Bytes::from(req).slice(0..1)[0], vec![exception])
}

const ILLEGAL_FUNCTION: u8 = 1;
const ILLEGAL_DATA_ADDRESS: u8 = 2;

struct ModbusService {
    tags: Tags,
}

impl ModbusService {
    pub fn new(tags: Tags) -> Self {
        ModbusService { tags }
    }
}

impl tokio_modbus::server::Service for ModbusService {
    type Request = tokio_modbus::prelude::Request;
    type Response = tokio_modbus::prelude::Response;
    type Error = std::io::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + Sync>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let holding_regs = &self.tags.holding_registers;
        let resp = match req {
            Self::Request::ReadHoldingRegisters(start, count) => holding_regs.get_array(|r| {
                if start as usize + count as usize <= holding_regs.len() {
                    let reg_slice = &r[(start as usize)..(start + count) as usize];
                    Ok(Self::Response::ReadHoldingRegisters(reg_slice.to_vec()))
                } else {
                    Ok(err_resp(req, ILLEGAL_DATA_ADDRESS))
                }
            }),
            Self::Request::WriteSingleRegister(addr, value) => {
               
                if (addr as usize) < holding_regs.len() {
                    debug!("Write: {} {}",addr, value);
                    holding_regs.update(addr as usize, &[value]);
                    Ok(Self::Response::WriteSingleRegister(addr, value))
                } else {
                    Ok(err_resp(req, ILLEGAL_DATA_ADDRESS))
                }
            }
            _ => Ok(err_resp(req, ILLEGAL_FUNCTION)),
        };

        Box::pin(future::ready(resp))
    }
}

struct ModbusNewService {
    tags: Tags,
}

impl ModbusNewService {
    pub fn new(tags: Tags) -> Self {
        ModbusNewService { tags }
    }
}

impl tokio_modbus::server::NewService for ModbusNewService {
    type Request = tokio_modbus::prelude::Request;
    type Response = tokio_modbus::prelude::Response;
    type Error = std::io::Error;
    type Instance = ModbusService;

    fn new_service(&self) -> std::io::Result<Self::Instance> {
        Ok(ModbusService::new(self.tags.clone()))
    }
}

pub async fn server_tcp(socket: SocketAddr, tags: Tags) -> DynResult<()> {
    let server = TcpServer::new(socket);
    let service = ModbusNewService::new(tags);
    match server.serve(service).await {
        Ok(_) => Ok(()),
        Err(e) => Err(Box::new(e)),
    }
}

pub async fn server_rtu<P>(path: P, baud_rate: u32, tags: Tags) -> DynResult<()> where P: AsRef<Path> {
    let server = RtuServer::new_from_path(path, baud_rate)?;
    let service = ModbusNewService::new(tags);
    server.serve_forever(service).await;
    Ok(())
}