use crate::devices::Devices;
use crate::error::DynResult;
use crate::observable_array::ObservableArray;
use crate::tags::Updated;
#[allow(unused_imports)]
use log::{debug, error};
use std::fmt::Debug;
use std::future::{self, Future};
use std::net::SocketAddr;
use std::ops::Range;
use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio::time::{self, Duration};
use tokio_modbus::ExceptionCode;
use tokio_modbus::client::Reader;
use tokio_modbus::client::{Context, rtu, tcp};
use tokio_modbus::prelude::SlaveContext;
use tokio_modbus::prelude::Writer;
use tokio_modbus::server::rtu::Server as RtuServer;
use tokio_modbus::server::tcp::Server as TcpServer;
use tokio_modbus::slave::Slave;
use tokio_serial::SerialStream;

struct ModbusService {
    devices: Devices,
}

impl ModbusService {
    pub fn new(devices: Devices) -> Self {
        ModbusService { devices }
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
    T: Default + Clone + Send + Sync + 'static,
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
    T: Default + Clone + Send + Sync + 'static,
{
    if (start as usize) < array.len() {
        array.update(start as usize, data);
        Ok(f(start, data))
    } else {
        Err(ExceptionCode::IllegalDataAddress)
    }
}

impl tokio_modbus::server::Service for ModbusService {
    type Request = tokio_modbus::SlaveRequest<'static>;
    type Response = tokio_modbus::Response;
    type Exception = ExceptionCode;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Exception>> + Send + Sync>>;

    fn call(&self, sreq: Self::Request) -> Self::Future {
        use tokio_modbus::Request::*;
        let tokio_modbus::SlaveRequest {
            slave: unit,
            request: req,
        } = sreq;
        let resp = match req {
            ReadHoldingRegisters(start, count) => self.devices.tags_read(unit, |tags| {
                server_read(
                    &tags.holding_registers,
                    start,
                    count,
                    |reply| Self::Response::ReadHoldingRegisters(reply),
                    &req,
                )
            }),
            WriteSingleRegister(addr, value) => self.devices.tags_read(unit, |tags| {
                server_write(
                    &tags.holding_registers,
                    addr,
                    &[value],
                    |addr, data| Self::Response::WriteSingleRegister(addr, data[0]),
                    &req,
                )
            }),
            WriteMultipleRegisters(addr, ref value) => self.devices.tags_read(unit, |tags| {
                server_write(
                    &tags.holding_registers,
                    addr,
                    value,
                    |addr, data| Self::Response::WriteMultipleRegisters(addr, data.len() as u16),
                    &req,
                )
            }),
            ReadInputRegisters(start, count) => self.devices.tags_read(unit, |tags| {
                server_read(
                    &tags.input_registers,
                    start,
                    count,
                    |reply| Self::Response::ReadInputRegisters(reply),
                    &req,
                )
            }),

            ReadCoils(start, count) => self.devices.tags_read(unit, |tags| {
                server_read(
                    &tags.coils,
                    start,
                    count,
                    |reply| Self::Response::ReadCoils(reply),
                    &req,
                )
            }),
            WriteSingleCoil(addr, value) => self.devices.tags_read(unit, |tags| {
                server_write(
                    &tags.coils,
                    addr,
                    &[value],
                    |addr, data| Self::Response::WriteSingleCoil(addr, data[0]),
                    &req,
                )
            }),
            WriteMultipleCoils(addr, ref value) => self.devices.tags_read(unit, |tags| {
                server_write(
                    &tags.coils,
                    addr,
                    value,
                    |addr, data| Self::Response::WriteMultipleCoils(addr, data.len() as u16),
                    &req,
                )
            }),
            ReadDiscreteInputs(start, count) => self.devices.tags_read(unit, |tags| {
                server_read(
                    &tags.discrete_inputs,
                    start,
                    count,
                    |reply| Self::Response::ReadDiscreteInputs(reply),
                    &req,
                )
            }),
            _ => Ok(Err(ExceptionCode::IllegalFunction)),
        };
        let resp = match resp {
            Ok(r) => r,
            Err(_) => Err(ExceptionCode::ServerDeviceFailure),
        };
        Box::pin(future::ready(resp))
    }
}

#[derive(Clone)]
pub struct ModbusOptions {
    pub poll_interval: Duration,
}

pub async fn server_tcp(
    socket: SocketAddr,
    devices: Devices,
    _options: ModbusOptions,
) -> DynResult<()> {
    let listener = TcpListener::bind(socket).await?;
    let server = TcpServer::new(listener);
    let on_connected =
        async |stream, _addr| Ok(Some((ModbusService::new(devices.clone()), stream)));
    let on_error = |error| {
        error!("Modbus processing failed: {}", error);
    };

    match server.serve(&on_connected, on_error).await {
        Ok(_) => Ok(()),
        Err(e) => Err(Box::new(e)),
    }
}

pub async fn server_rtu(
    ser: SerialStream,
    devices: Devices,
    _options: ModbusOptions,
) -> DynResult<()> {
    let server = RtuServer::new(ser);
    let service = ModbusService::new(devices);
    server.serve_forever(service).await?;
    Ok(())
}

enum ClientOp {
    ReadHoldingRegisters(u8, u16, u16),
    //WriteHoldingRegisters(u16, u16),
    ReadInputRegisters(u8, u16, u16),
    ReadCoils(u8, u16, u16),
    //WriteCoils(u16, u16),
    ReadDiscreteInputs(u8, u16, u16),
}

const READ_BITS_MAX_LEN: u16 = 2000;
const READ_REGISTERS_MAX_LEN: u16 = 125;
//const WRITE_REGISTERS_MAX_LEN: u16 = 123;
//const WRITE_BITS_MAX_LEN: u16 = 1968;

const CLIENT_TIMEOUT: Duration = Duration::from_millis(500);

impl ClientOp {
    pub async fn execute(&self, client: &mut Context, devices: &Devices) -> DynResult<()> {
        match self {
            ClientOp::ReadHoldingRegisters(unit, start, length) => {
                match tokio::time::timeout(CLIENT_TIMEOUT, {
                    client.set_slave(Slave(*unit));
                    client.read_holding_registers(*start, *length)
                })
                .await
                {
                    Ok(Ok(Ok(data))) => {
                        devices.tags_write(*unit, |tags| {
                            tags.holding_registers.update(*start as usize, &data);
                        })?;
                    }
                    Ok(Ok(Err(code))) => return Err(code.into()),
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
            ClientOp::ReadInputRegisters(unit, start, length) => {
                match tokio::time::timeout(CLIENT_TIMEOUT, {
                    client.set_slave(Slave(*unit));
                    client.read_input_registers(*start, *length)
                })
                .await
                {
                    Ok(Ok(Ok(data))) => devices.tags_write(*unit, |tags| {
                        tags.input_registers.update(*start as usize, &data);
                    })?,
                    Ok(Ok(Err(code))) => return Err(code.into()),
                    Ok(Err(e)) => return Err(e.into()),
                    Err(e) => return Err(e.into()),
                }
            }
            ClientOp::ReadCoils(unit, start, length) => {
                match tokio::time::timeout(CLIENT_TIMEOUT, {
                    client.set_slave(Slave(*unit));
                    client.read_coils(*start, *length)
                })
                .await
                {
                    Ok(Ok(Ok(data))) => {
                        devices.tags_write(*unit, |tags| {
                            tags.coils.update(*start as usize, &data);
                        })?;
                    }
                    Ok(Ok(Err(code))) => return Err(code.into()),
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
            ClientOp::ReadDiscreteInputs(unit, start, length) => {
                match tokio::time::timeout(CLIENT_TIMEOUT, {
                    client.set_slave(Slave(*unit));
                    client.read_discrete_inputs(*start, *length)
                })
                .await
                {
                    Ok(Ok(Ok(data))) => {
                        devices.tags_write(*unit, |tags| {
                            tags.discrete_inputs.update(*start as usize, &data);
                        })?;
                    }
                    Ok(Ok(Err(code))) => return Err(code.into()),
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

    pub fn read_sequence(devices: &Devices) -> Vec<ClientOp> {
        let mut seq = Vec::new();
        for unit in devices.units() {
            let ranges = devices.ranges(unit).unwrap();
            for range in &ranges.holding_registers {
                Self::push_range(&mut seq, range, READ_REGISTERS_MAX_LEN, |start, length| {
                    ClientOp::ReadHoldingRegisters(unit, start, length)
                });
            }
            for range in &ranges.input_registers {
                Self::push_range(&mut seq, range, READ_REGISTERS_MAX_LEN, |start, length| {
                    ClientOp::ReadInputRegisters(unit, start, length)
                });
            }
            for range in &ranges.coils {
                Self::push_range(&mut seq, range, READ_BITS_MAX_LEN, |start, length| {
                    ClientOp::ReadCoils(unit, start, length)
                });
            }
            for range in &ranges.discrete_inputs {
                Self::push_range(&mut seq, range, READ_BITS_MAX_LEN, |start, length| {
                    ClientOp::ReadDiscreteInputs(unit, start, length)
                });
            }
        }
        seq
    }
}

async fn handle_poll(
    unit: u8,
    updated: &Updated,
    client: &mut Context,
    devices: &Devices,
) -> DynResult<()> {
    use Updated::*;
    match updated {
        HoldingRegisters(changes) => {
            for range in changes {
                let start = range.start;
                let length = range.end - range.start;
                let data = devices.tags_read(unit, |tags| {
                    tags.holding_registers
                        .get_array(|r| Vec::from(&r[start..start + length]))
                })?;
                if length == 1 {
                    client
                        .write_single_register(start as u16, data[0])
                        .await??;
                } else {
                    client
                        .write_multiple_registers(start as u16, &data)
                        .await??;
                }
            }
        }
        Coils(changes) => {
            for range in changes {
                let start = range.start;
                let length = range.end - range.start;
                let data = devices.tags_read(unit, |tags| {
                    tags.coils
                        .get_array(|r| Vec::from(&r[start..start + length]))
                })?;

                if length == 1 {
                    client.write_single_coil(start as u16, data[0]).await??;
                } else {
                    client.write_multiple_coils(start as u16, &data).await??;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

async fn client_poll(
    client: &mut Context,
    devices: Devices,
    options: &ModbusOptions,
) -> DynResult<()> {
    let seq = ClientOp::read_sequence(&devices);
    let mut iter = seq.iter().cycle();
    loop {
        let op = iter.next().unwrap();
        if let Err(e) = op.execute(client, &devices).await {
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
            (unit, updated) = devices.updated() => {
		if let Err(e) = handle_poll(unit, &updated, client, &devices).await {
		    error!("Failed to send data to server: {e}");
		}
            }
        }
    }
}

pub async fn client_rtu<T>(
    ser: T,
    slave: Slave,
    devices: Devices,
    options: ModbusOptions,
) -> DynResult<()>
where
    T: AsyncRead + AsyncWrite + Debug + Unpin + Send + 'static,
{
    let mut ctxt = rtu::attach_slave(ser, slave);
    client_poll(&mut ctxt, devices, &options).await?;
    Ok(())
}

pub async fn client_tcp(
    socket: SocketAddr,
    devices: Devices,
    options: ModbusOptions,
) -> DynResult<()> {
    loop {
        match tcp::connect_slave(socket, Slave(0)).await {
            Ok(mut ctxt) => {
                if let Err(e) = client_poll(&mut ctxt, devices.clone(), &options).await {
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
