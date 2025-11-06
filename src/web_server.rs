use crate::error::DynResult;
use bytes::Bytes;
use futures::future;
use futures::future::Either;
use futures::future::Fuse;
use futures::FutureExt;
use futures::SinkExt;
use futures::StreamExt;
use http_body_util::Full;
use hyper::header;
use hyper::http::StatusCode;
use hyper::service::service_fn;
use hyper::Method;
use hyper::{
    body::{Body, Incoming},
    server::conn::http1,
    Request, Response,
};
use tungstenite::protocol::Message as WsMessage;
use tungstenite::Utf8Bytes;

use hyper_tungstenite::HyperWebsocket;
use hyper_util::rt::TokioIo;
#[allow(unused_imports)]
use log::{debug, error, info};
use std::convert::Infallible;
use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

pub type DynBody = Box<dyn Body<Data = Bytes, Error = Infallible> + Send + Sync + Unpin>;
pub type DynResponse = Response<DynBody>;
pub type BuildPage = Box<dyn Fn(Request<Incoming>) -> DynResult<DynResponse> + Send + Sync>;

/// Takes a path and returns (mime_type, resource_data)
pub type GetResurce = Box<dyn Fn(&str) -> DynResult<(&str, Bytes)> + Send + Sync>;

fn into_dyn_response<T>(resp: Response<T>) -> DynResponse
where
    T: Body<Data = Bytes, Error = Infallible> + Send + Sync + Unpin + 'static,
{
    let (parts, body) = resp.into_parts();
    Response::from_parts(parts, Box::new(body))
}

pub trait WebsocketConnect {
    fn connected(&self, send: WsSender) -> Box<dyn WebsocketReceive + Send + Sync>;
}

pub trait WebsocketReceive {
    fn message(&mut self, msg: &str) -> Option<String>;
    fn disconnected(&mut self);
}

pub struct ServerConfig {
    bind_addr: Option<IpAddr>,
    port: Option<u16>,
    build_page: BuildPage,
    web_resource: GetResurce,
    ws_connect: Box<dyn WebsocketConnect + Sync + Send>,
}

fn no_resource(_path: &str) -> DynResult<(&str, Bytes)> {
    Err("No resource".into())
}
impl ServerConfig {
    pub fn new(ws_connect: Box<dyn WebsocketConnect + Sync + Send>) -> Self {
        Self {
            bind_addr: None,
            port: None,
            build_page: Box::new(default_page),
            web_resource: Box::new(no_resource),
            ws_connect,
        }
    }

    pub fn port(mut self, p: u16) -> Self {
        self.port = Some(p);
        self
    }
    pub fn bind_addr(mut self, a: IpAddr) -> Self {
        self.bind_addr = Some(a);
        self
    }

    pub fn build_page(mut self, f: BuildPage) -> Self {
        self.build_page = f;
        self
    }

    pub fn web_resource(mut self, resource: GetResurce) -> Self {
        self.web_resource = resource;
        self
    }
}

pub fn default_page(_req: Request<Incoming>) -> DynResult<DynResponse> {
    Ok(Response::new(Box::new("Hello World".to_string()) as DynBody))
}

pub type WsSender = mpsc::Sender<String>;

pub async fn ws_client(ws: HyperWebsocket, conf: Arc<ServerConfig>) {
    info!("Connecting WS");
    let (ws_send_in, mut ws_send_out) = mpsc::channel::<String>(4);
    let mut stream = match ws.await {
        Ok(s) => s,
        Err(e) => {
            error!("Upgrading connection failed: {e}");
            return;
        }
    };
    info!("Connected WS");
    let mut recv_handler = conf.ws_connect.connected(ws_send_in);
    let mut send_closed = false;
    loop {
        let wait_send = if send_closed {
	    Fuse::terminated()
	}
	else {
	    ws_send_out.recv().fuse()
	};
	tokio::pin!(wait_send);
        match future::select(wait_send, stream.next()).await {
            Either::Left((Some(msg), _)) => {
                if let Err(e) = stream.send(WsMessage::Text(Utf8Bytes::from(msg))).await {
                    error!("Failed to send message over websocket: {e}");
                }
            }
            Either::Left((None, _)) => {
                debug!("Websocket send pipe closed");
                send_closed = true;
            }
            Either::Right((Some(msg), _)) => {
                debug!("Got message: {msg:?}");
                match msg {
                    Ok(WsMessage::Text(bytes)) => {
                        if let Some(reply) = recv_handler.message(bytes.as_str()) {
                            if let Err(e) =
                                stream.send(WsMessage::Text(Utf8Bytes::from(reply))).await
                            {
                                error!("Failed to send message over websocket: {e}");
                            }
                        }
                    }
                    Err(e) => {
                        error!("Message error: {e}");
                    }
                    _ => {}
                }
            }
            Either::Right((None, _)) => {
		recv_handler.disconnected();
                break;
            }
        }
    }
    /*
        loop {
            tokio::select! {
                res = client.next() => {
                    if let Some(msg) = res {
                        if let Ok(msg) = msg {
                            if msg.opcode() == Opcode::Text {
                                if let Err(e) = ws_send.send(msg.into_data()).await {
                                    error!("Failed to send WS message to handler: {}",e)
                                }
                            }
                        }
                    } else {
                        break;
                    }
                }
                Ok(data) = ws_receive.recv() => {
                    match Message::new(Opcode::Text, data) {
                        Ok(msg) => {
                            if let Err(e) = client.send(msg).await {
                                error!("Failed to send message to WS client: {}",e);
                            }
                        }
                        Err(e) => {
                            error!("Failed to create message to WS client: {}",e);
                        }
                    }


                }
            }
    }
        */
    info!("Client disconnected")
}

async fn handle(conf: Arc<ServerConfig>, mut req: Request<Incoming>) -> DynResult<DynResponse> {
    let path = req.uri().path();
    match req.method() {
        &Method::GET => {
            if path.starts_with("/dyn/") {
                debug!("Requested dyn");
                (conf.build_page)(req)
            } else if path.starts_with("/socket/") {
                debug!("Requested socket");

                if hyper_tungstenite::is_upgrade_request(&req) {
                    let (response, websocket) = hyper_tungstenite::upgrade(&mut req, None)?;

                    tokio::spawn(ws_client(websocket, conf));
                    Ok(into_dyn_response(response))
                } else {
                    Ok(into_dyn_response(
                        Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .header(header::CONTENT_TYPE, "text/plain")
                            .body(Full::from(format!("Expected websocket upgrade")))?,
                    ))
                }
            } else {
                debug!("Requested resource {}", req.uri().path());
                let (mime_type, data) = {
                    match (conf.web_resource)(req.uri().path()) {
                        Ok(res) => res,
                        Err(e) => {
                            return Ok(into_dyn_response(
                                Response::builder()
                                    .status(StatusCode::NOT_FOUND)
                                    .header(header::CONTENT_TYPE, "text/plain")
                                    .body(Full::from(format!("File error: {e}")))?,
                            ))
                        }
                    }
                };
                Ok(into_dyn_response(
                    Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, mime_type)
                        .body(Full::from(data))?,
                ))
            }
        }
        m => Ok(into_dyn_response(
            Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .header(header::CONTENT_TYPE, "text/plain")
                .body(Full::from(format!("Method {m} not supported")))?,
        )),
    }
}
pub fn setup_server(
    conf: ServerConfig,
) -> DynResult<(
    Pin<Box<dyn Future<Output = DynResult<()>> + Send + Sync>>,
    IpAddr,
    u16,
)> {
    let port = conf.port.unwrap_or(0);
    let bind_addr = conf
        .bind_addr
        .unwrap_or_else(|| IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
    let socket_addr = SocketAddr::new(bind_addr, port);
    let conf = Arc::new(conf);
    let service = service_fn(move |req| handle(conf.clone(), req));
    let server = Box::pin(async move {
        let listener = TcpListener::bind(socket_addr).await?;
        loop {
            {
                let (stream, _) = listener.accept().await?;
                let io = TokioIo::new(stream);
                let service = service.clone();
                tokio::spawn(async move {
                    if let Err(err) = http1::Builder::new()
                        .serve_connection(io, service)
                        .with_upgrades()
                        .await
                    {
                        error!("Error serving connection: {:?}", err);
                    }
                });
            }
        }
    });
    Ok((server, bind_addr, port))
}
