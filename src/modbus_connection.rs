use crate::error::DynResult;
use crate::observable_array::ObservableArray;
use crate::tag_ranges::TagRanges;
use crate::tags::Tags;
#[allow(unused_imports)]
use log::{debug, error};
use std::fmt::Debug;
use std::future::{self, Future};
use std::net::SocketAddr;
use std::ops::Range;
use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::time::{self, Duration};
use tokio_modbus::client::Reader;
use tokio_modbus::client::Writer;
use tokio_modbus::client::{rtu, tcp, Context};
use tokio_modbus::server::rtu::Server as RtuServer;
use tokio_modbus::server::tcp::Server as TcpServer;
use tokio_modbus::slave::Slave;
use tokio_modbus::ExceptionCode;
use tokio_serial::SerialStream;

/*
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
 */

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
    _req: &tokio_modbus::prelude::Request,
) -> Result<tokio_modbus::prelude::Response, ExceptionCode>
where
    F: FnOnce(Vec<T>) -> tokio_modbus::prelude::Response,
    T: Default + Clone,
{
    array.get_array(|r| {
        if start as usize + count as usize <= array.len() {
            let reg_slice = &r[(start as usize)..(start + count) as usize];
            Ok(f(reg_slice.to_vec()))
        } else {
            Err(ExceptionCode::IllegalDataAddress)
        }
    })
}

fn server_write<T, F>(
    array: &ObservableArray<T>,
    start: u16,
    data: &[T],
    f: F,
    _req: &tokio_modbus::prelude::Request,
) -> Result<tokio_modbus::prelude::Response, ExceptionCode>
where
    F: FnOnce(u16, &[T]) -> tokio_modbus::prelude::Response,
    T: Default + Clone,
{
    if (start as usize) < array.len() {
        array.update(start as usize, data);
        Ok(f(start, data))
    } else {
        Err(ExceptionCode::IllegalDataAddress)
    }
}

impl tokio_modbus::server::Service for ModbusService {
    type Request = tokio_modbus::prelude::SlaveRequest<'static>;
    type Response = tokio_modbus::prelude::Response;
    type Exception = ExceptionCode;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Exception>> + Send + Sync>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        use tokio_modbus::prelude::Request::*;
        let resp = match req.request {
            ReadHoldingRegisters(start, count) => server_read(
                &self.tags.holding_registers,
                start,
                count,
                |reply| Self::Response::ReadHoldingRegisters(reply),
                &req.request,
            ),
            WriteSingleRegister(addr, value) => server_write(
                &self.tags.holding_registers,
                addr,
                &[value],
                |addr, data| Self::Response::WriteSingleRegister(addr, data[0]),
                &req.request,
            ),
            WriteMultipleRegisters(addr, ref value) => server_write(
                &self.tags.holding_registers,
                addr,
                value,
                |addr, data| Self::Response::WriteMultipleRegisters(addr, data.len() as u16),
                &req.request,
            ),
            ReadInputRegisters(start, count) => server_read(
                &self.tags.input_registers,
                start,
                count,
                |reply| Self::Response::ReadInputRegisters(reply),
                &req.request,
            ),

            ReadCoils(start, count) => server_read(
                &self.tags.coils,
                start,
                count,
                |reply| Self::Response::ReadCoils(reply),
                &req.request,
            ),
            WriteSingleCoil(addr, value) => server_write(
                &self.tags.coils,
                addr,
                &[value],
                |addr, data| Self::Response::WriteSingleCoil(addr, data[0]),
                &req.request,
            ),
            WriteMultipleCoils(addr, ref value) => server_write(
                &self.tags.coils,
                addr,
                value,
                |addr, data| Self::Response::WriteMultipleCoils(addr, data.len() as u16),
                &req.request,
            ),
            ReadDiscreteInputs(start, count) => server_read(
                &self.tags.discrete_inputs,
                start,
                count,
                |reply| Self::Response::ReadDiscreteInputs(reply),
                &req.request,
            ),
            _ => Err(ExceptionCode::IllegalFunction),
        };

        Box::pin(future::ready(resp))
    }
}

/*
struct ModbusNewService {
    tags: Tags,
}

impl ModbusNewService {
    pub fn new(tags: Tags) -> Self {
        ModbusNewService { tags }
    }
}

impl tokio_modbus::server::NewService for ModbusNewService {
    type Request = tokio_modbus::prelude::SlaveRequest<'static>;
    type Response = tokio_modbus::prelude::Response;
    type Error = std::io::Error;
    type Instance = ModbusService;

    fn new_service(&self) -> std::io::Result<Self::Instance> {
        Ok(ModbusService::new(self.tags.clone()))
    }
}
*/
#[derive(Clone)]
pub struct ModbusOptions {
    pub poll_interval: Duration,
}

pub async fn server_tcp(socket: SocketAddr, tags: Tags, _options: ModbusOptions) -> DynResult<()> {
    let listener = TcpListener::bind(socket).await?;
    let server = TcpServer::new(listener);
    let on_connected = async |stream: TcpStream, _addr: SocketAddr| {
        Ok(Some((ModbusService::new(tags.clone()), stream)))
    };
    let on_process_error = |error| {
        error!("Process error: {}", error);
    };

    match server.serve(&on_connected, on_process_error).await {
        Ok(_) => Ok(()),
        Err(e) => Err(Box::new(e)),
    }
}

pub async fn server_rtu(
    ser: SerialStream,
    tags: Tags,
    _ranges: TagRanges,
    _options: ModbusOptions,
) -> DynResult<()> {
    let server = RtuServer::new(ser);
    let service = ModbusService::new(tags);
    let _ = server.serve_forever(service).await;
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

const CLIENT_TIMEOUT: Duration = Duration::from_millis(500);

impl ClientOp {
    pub async fn execute(&self, client: &mut Context, tags: &mut Tags) -> DynResult<()> {
        match self {
            ClientOp::ReadHoldingRegisters(start, length) => {
                match tokio::time::timeout(
                    CLIENT_TIMEOUT,
                    client.read_holding_registers(*start, *length),
                )
                .await
                {
                    Ok(Ok(Ok(data))) => {
                        tags.holding_registers.update(*start as usize, &data);
                    }
                    Ok(Ok(Err(e))) => return Err(e.into()),
                    Ok(Err(e)) => return Err(e.into()),
                    Err(e) => return Err(e.into()),
                }
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
                match tokio::time::timeout(
                    CLIENT_TIMEOUT,
                    client.read_input_registers(*start, *length),
                )
                .await
                {
                    Ok(Ok(Ok(data))) => {
                        tags.input_registers.update(*start as usize, &data);
                    }
                    Ok(Ok(Err(e))) => return Err(e.into()),
                    Ok(Err(e)) => return Err(e.into()),
                    Err(e) => return Err(e.into()),
                }
            }
            ClientOp::ReadCoils(start, length) => {
                match tokio::time::timeout(CLIENT_TIMEOUT, client.read_coils(*start, *length)).await
                {
                    Ok(Ok(Ok(data))) => {
                        tags.coils.update(*start as usize, &data);
                    }
                    Ok(Ok(Err(e))) => return Err(e.into()),
                    Ok(Err(e)) => return Err(e.into()),
                    Err(e) => return Err(e.into()),
                }
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
                match tokio::time::timeout(
                    CLIENT_TIMEOUT,
                    client.read_discrete_inputs(*start, *length),
                )
                .await
                {
                    Ok(Ok(Ok(data))) => {
                        tags.discrete_inputs.update(*start as usize, &data);
                    }
                    Ok(Ok(Err(e))) => return Err(e.into()),
                    Ok(Err(e)) => return Err(e.into()),
                    Err(e) => return Err(e.into()),
                }
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

async fn client_poll(
    client: &mut Context,
    tags: &mut Tags,
    ranges: &TagRanges,
    options: &ModbusOptions,
) -> DynResult<()> {
    let seq = ClientOp::read_sequence(&ranges);
    let mut iter = seq.iter().cycle();
    loop {
        let op = iter.next().unwrap();
        if let Err(e) = op.execute(client, tags).await {
            error!("Failed to read from server: {e}");
            if let Ok(io_err) = e.downcast::<std::io::Error>() {
                if let std::io::ErrorKind::BrokenPipe = io_err.kind() {
                    debug!("Error: {io_err:?}");
                    return Err(io_err);
                }
            }
        }
        tokio::select! {
            _res = time::sleep(options.poll_interval) => (),
        changes = tags.holding_registers.updated() => {
        for range in &changes {
                    let start = range.start;
                    let length = range.end - range.start;
                    let data = tags
            .holding_registers
            .get_array(|r| Vec::from(&r[start..start + length]));
                        if length == 1 {
                client.write_single_register(start as u16, data[0]).await??;
                        } else {
                client.write_multiple_registers(start as u16, &data).await??;
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
                client.write_single_coil(start as u16, data[0]).await??;
                        } else {
                client.write_multiple_coils(start as u16, &data).await??;
                        }
        }
            }
        }
    }
}

pub async fn client_rtu<T>(
    ser: T,
    slave: Slave,
    mut tags: Tags,
    ranges: TagRanges,
    options: ModbusOptions,
) -> DynResult<()>
where
    T: AsyncRead + AsyncWrite + Debug + Unpin + Send + 'static,
{
    let mut ctxt = rtu::attach_slave(ser, slave);
    client_poll(&mut ctxt, &mut tags, &ranges, &options).await?;
    Ok(())
}

pub async fn client_tcp(
    socket: SocketAddr,
    slave: Slave,
    mut tags: Tags,
    ranges: TagRanges,
    options: ModbusOptions,
) -> DynResult<()> {
    loop {
        match tcp::connect_slave(socket, slave).await {
            Ok(mut ctxt) => {
                if let Err(e) = client_poll(&mut ctxt, &mut tags, &ranges, &options).await {
                    if let Ok(io_err) = e.downcast::<std::io::Error>() {
                        if let std::io::ErrorKind::BrokenPipe = io_err.kind() {
                        } else {
                            break;
                        }
                    }
                }
            }
            Err(_e) => {}
        };
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    Ok(())
}
