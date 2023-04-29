use crate::error::DynResult;
use crate::observable_array::ObservableArray;
use crate::tag_ranges::TagRanges;
use crate::tags::Tags;
use bytes::Bytes;
#[allow(unused_imports)]
use log::{debug, error};
use std::fmt::Debug;
use std::future::{self, Future};
use std::net::SocketAddr;
use std::ops::Range;
use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::{self, Duration};
use tokio_modbus::client::Reader;
use tokio_modbus::client::Writer;
use tokio_modbus::client::{rtu, tcp, Context};
use tokio_modbus::server::rtu::Server as RtuServer;
use tokio_modbus::server::tcp::Server as TcpServer;
use tokio_modbus::slave::Slave;
use tokio_serial::SerialStream;

fn err_resp(
    req: &tokio_modbus::prelude::Request,
    exception: u8,
) -> tokio_modbus::prelude::Response {
    tokio_modbus::prelude::Response::Custom(
        0x80 | Bytes::from(req.clone()).slice(0..1)[0],
        vec![exception],
    )
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

fn server_read<T, F>(
    array: &ObservableArray<T>,
    start: u16,
    count: u16,
    f: F,
    req: &tokio_modbus::prelude::Request,
) -> Result<tokio_modbus::prelude::Response, std::io::Error>
where
    F: FnOnce(Vec<T>) -> tokio_modbus::prelude::Response,
    T: Default + Clone,
{
    array.get_array(|r| {
        if start as usize + count as usize <= array.len() {
            let reg_slice = &r[(start as usize)..(start + count) as usize];
            Ok(f(reg_slice.to_vec()))
        } else {
            Ok(err_resp(req, ILLEGAL_DATA_ADDRESS))
        }
    })
}

fn server_write<T, F>(
    array: &ObservableArray<T>,
    start: u16,
    data: &[T],
    f: F,
    req: &tokio_modbus::prelude::Request,
) -> Result<tokio_modbus::prelude::Response, std::io::Error>
where
    F: FnOnce(u16, &[T]) -> tokio_modbus::prelude::Response,
    T: Default + Clone,
{
    if (start as usize) < array.len() {
        array.update(start as usize, data);
        Ok(f(start, data))
    } else {
        Ok(err_resp(req, ILLEGAL_DATA_ADDRESS))
    }
}

impl tokio_modbus::server::Service for ModbusService {
    type Request = tokio_modbus::prelude::Request;
    type Response = tokio_modbus::prelude::Response;
    type Error = std::io::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + Sync>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let resp = match req {
            Self::Request::ReadHoldingRegisters(start, count) => server_read(
                &self.tags.holding_registers,
                start,
                count,
                |reply| Self::Response::ReadHoldingRegisters(reply),
                &req,
            ),
            Self::Request::WriteSingleRegister(addr, value) => server_write(
                &self.tags.holding_registers,
                addr,
                &[value],
                |addr, data| Self::Response::WriteSingleRegister(addr, data[0]),
                &req,
            ),
            Self::Request::WriteMultipleRegisters(addr, ref value) => server_write(
                &self.tags.holding_registers,
                addr,
                value,
                |addr, data| Self::Response::WriteMultipleRegisters(addr, data.len() as u16),
                &req,
            ),
            Self::Request::ReadInputRegisters(start, count) => server_read(
                &self.tags.input_registers,
                start,
                count,
                |reply| Self::Response::ReadInputRegisters(reply),
                &req,
            ),

            Self::Request::ReadCoils(start, count) => server_read(
                &self.tags.coils,
                start,
                count,
                |reply| Self::Response::ReadCoils(reply),
                &req,
            ),
            Self::Request::WriteSingleCoil(addr, value) => server_write(
                &self.tags.coils,
                addr,
                &[value],
                |addr, data| Self::Response::WriteSingleCoil(addr, data[0]),
                &req,
            ),
            Self::Request::WriteMultipleCoils(addr, ref value) => server_write(
                &self.tags.coils,
                addr,
                value,
                |addr, data| Self::Response::WriteMultipleCoils(addr, data.len() as u16),
                &req,
            ),
            Self::Request::ReadDiscreteInputs(start, count) => server_read(
                &self.tags.discrete_inputs,
                start,
                count,
                |reply| Self::Response::ReadDiscreteInputs(reply),
                &req,
            ),
            _ => Ok(err_resp(&req, ILLEGAL_FUNCTION)),
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

pub async fn server_rtu(ser: SerialStream, tags: Tags, _ranges: TagRanges) -> DynResult<()> {
    let server = RtuServer::new(ser);
    let service = ModbusNewService::new(tags);
    server.serve_forever(service).await;
    Ok(())
}

enum ClientOp {
    ReadHoldingRegisters(u16, u16),
    //WriteHoldingRegisters(u16, u16),
    ReadInputRegisters(u16, u16),
    ReadCoils(u16, u16),
    //WriteCoils(u16, u16),
    ReadDiscreteInputs(u16, u16),
}

const READ_BITS_MAX_LEN: u16 = 2000;
const READ_REGISTERS_MAX_LEN: u16 = 125;
//const WRITE_REGISTERS_MAX_LEN: u16 = 123;
//const WRITE_BITS_MAX_LEN: u16 = 1968;

impl ClientOp {
    pub async fn execute(&self, client: &mut Context, tags: &mut Tags) -> DynResult<()> {
        match self {
            ClientOp::ReadHoldingRegisters(start, length) => {
                let data = client.read_holding_registers(*start, *length).await?;
                tags.holding_registers.update(*start as usize, &data);
            }
            /*
            ClientOp::WriteHoldingRegisters(start, length) => {
                let data = tags
                    .holding_registers
                    .get_array(|r| Vec::from(&r[*start as usize..(*start + *length) as usize]));
                if *length == 1 {
                    client.write_single_register(*start, data[0]).await?;
                } else {
                    client.write_multiple_registers(*start, &data).await?;
                }
            }*/
            ClientOp::ReadInputRegisters(start, length) => {
                let data = client.read_input_registers(*start, *length).await?;
                tags.input_registers.update(*start as usize, &data);
            }
            ClientOp::ReadCoils(start, length) => {
                let data = client.read_coils(*start, *length).await?;
                tags.coils.update(*start as usize, &data);
            }
            /*
            ClientOp::WriteCoils(start, length) => {
                let data = tags
                    .coils
                    .get_array(|r| Vec::from(&r[*start as usize..(*start + *length) as usize]));
                if *length == 1 {
                    client.write_single_coil(*start, data[0]).await?;
                } else {
                    client.write_multiple_coils(*start, &data).await?;
                }
            }*/
            ClientOp::ReadDiscreteInputs(start, length) => {
                let data = client.read_discrete_inputs(*start, *length).await?;
                tags.discrete_inputs.update(*start as usize, &data);
            }
        }
        Ok(())
    }

    fn push_range<F>(seq: &mut Vec<ClientOp>, range: &Range<u16>, max_len: u16, f: F)
    where
        F: Fn(u16, u16) -> ClientOp,
    {
        let mut start = range.start;
        let mut length = range.end - range.start;
        while start < range.end {
            let op_len = length.min(max_len);
            seq.push(f(start, op_len));
            start += op_len;
            length -= op_len;
        }
    }

    pub fn read_sequence(ranges: &TagRanges) -> Vec<ClientOp> {
        let mut seq = Vec::new();
        for range in &ranges.holding_registers {
            Self::push_range(&mut seq, range, READ_REGISTERS_MAX_LEN, |start, length| {
                ClientOp::ReadHoldingRegisters(start, length)
            });
        }
        for range in &ranges.input_registers {
            Self::push_range(&mut seq, range, READ_REGISTERS_MAX_LEN, |start, length| {
                ClientOp::ReadInputRegisters(start, length)
            });
        }
        for range in &ranges.coils {
            Self::push_range(&mut seq, range, READ_BITS_MAX_LEN, |start, length| {
                ClientOp::ReadCoils(start, length)
            });
        }
        for range in &ranges.discrete_inputs {
            Self::push_range(&mut seq, range, READ_BITS_MAX_LEN, |start, length| {
                ClientOp::ReadDiscreteInputs(start, length)
            });
        }

        seq
    }
}

async fn client_poll(client: &mut Context, mut tags: Tags, ranges: TagRanges) -> DynResult<()> {
    let seq = ClientOp::read_sequence(&ranges);
    let mut iter = seq.iter().cycle();
    loop {
        let op = iter.next().unwrap();
        if let Err(e) = op.execute(client, &mut tags).await {
            error!("Failed to read from server: {e}");
        }
        tokio::select! {
            _res = time::sleep(Duration::from_millis(921)) => (),
        changes = tags.holding_registers.updated() => {
        for range in &changes {
                    let start = range.start;
                    let length = range.end - range.start;
                    let data = tags
            .holding_registers
            .get_array(|r| Vec::from(&r[start..start + length]));
                        if length == 1 {
                client.write_single_register(start as u16, data[0]).await?;
                        } else {
                client.write_multiple_registers(start as u16, &data).await?;
                        }
        }
            }
            changes = tags.coils.updated() => {
        for range in &changes {
                    let start = range.start;
                    let length = range.end - range.start;
                    let data = tags
            .coils
            .get_array(|r| Vec::from(&r[start..start + length]));
                        if length == 1 {
                client.write_single_coil(start as u16, data[0]).await?;
                        } else {
                client.write_multiple_coils(start as u16, &data).await?;
                        }
        }
            }
        }
    }
}

pub async fn client_rtu<T>(ser: T, slave: Slave, tags: Tags, ranges: TagRanges) -> DynResult<()>
where
    T: AsyncRead + AsyncWrite + Debug + Unpin + Send + 'static,
{
    let mut ctxt = rtu::connect_slave(ser, slave).await?;
    client_poll(&mut ctxt, tags, ranges).await?;
    Ok(())
}

pub async fn client_tcp(
    socket: SocketAddr,
    slave: Slave,
    tags: Tags,
    ranges: TagRanges,
) -> DynResult<()> {
    let mut ctxt = tcp::connect_slave(socket, slave).await?;
    client_poll(&mut ctxt, tags, ranges).await?;
    Ok(())
}
