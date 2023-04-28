use crate::error::DynResult;
use bytes::Bytes;
use futures_util::sink::SinkExt;
use futures_util::StreamExt;
use hyper::header;
use hyper::http::StatusCode;
use hyper::service::{make_service_fn, service_fn};
use hyper::Method;
use hyper::{Body, Request, Response, Server};
use hyper_websocket_lite::AsyncClient;
#[allow(unused_imports)]
use log::{debug, error, info};
use std::convert::Infallible;
use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc};
use websocket_lite::{Message, Opcode};

pub type BuildPage = Box<dyn FnMut(Request<Body>) -> DynResult<Response<Body>> + Send>;

pub struct ServerConfig {
    bind_addr: Option<IpAddr>,
    port: Option<u16>,
    build_page: BuildPage,
    web_root: PathBuf,
    ws_send: WsSender,
    ws_receive: WsReceiveChannel,
}

impl ServerConfig {
    pub fn new(ws_send: WsSender, ws_receive: WsReceiveChannel) -> Self {
        Self {
            bind_addr: None,
            port: None,
            build_page: Box::new(default_page),
            web_root: PathBuf::from("web"),
            ws_send,
            ws_receive,
        }
    }

    pub fn port(mut self, p: u16) -> Self {
        self.port = Some(p);
        self
    }
    pub fn build_page(mut self, f: BuildPage) -> Self {
        self.build_page = f;
        self
    }

    pub fn web_root(mut self, root: PathBuf) -> Self {
        self.web_root = root;
        self
    }
}

pub fn default_page(_req: Request<Body>) -> DynResult<Response<Body>> {
    Ok(Response::new(Body::from("Hello World")))
}

type WsSender = mpsc::Sender<Bytes>;
type WsReceiver = broadcast::Receiver<Bytes>;
type WsReceiveChannel = broadcast::Sender<Bytes>;

pub async fn ws_client(mut client: AsyncClient, ws_send: WsSender, mut ws_receive: WsReceiver) {
    info!("Connected WS");
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
    info!("Client disconnected")
}

async fn handle(conf: Arc<Mutex<ServerConfig>>, req: Request<Body>) -> DynResult<Response<Body>> {
    let path = req.uri().path();
    match req.method() {
        &Method::GET => {
            if path.starts_with("/dyn/") {
                let mut conf = conf.lock().unwrap();
                (conf.build_page)(req)
            } else if path.starts_with("/socket/") {
                let (ws_send, ws_receive) = {
                    let conf = conf.lock().unwrap();
                    (conf.ws_send.clone(), conf.ws_receive.subscribe())
                };

                hyper_websocket_lite::server_upgrade(req, |client| {
                    ws_client(client, ws_send, ws_receive)
                })
                .await
            } else {
                let files = {
                    let conf = conf.lock().unwrap();
                    hyper_staticfile::Static::new(conf.web_root.clone())
                };
                files
                    .serve(req)
                    .await
                    .or_else(|e| {
                        Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .header(header::CONTENT_TYPE, "text/plain")
                            .body(Body::from(format!("File error: {e}")))
                    })
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>)
            }
        }
        m => Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(Body::from(format!("Method {m} not supported")))
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
    }
}
pub fn setup_server(conf: ServerConfig) -> (impl Future<Output = Result<(), hyper::Error>>, u16)
{
    let port = conf.port.unwrap_or(0);
    let bind_addr = conf
        .bind_addr
        .unwrap_or_else(|| IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
    let socket_addr = SocketAddr::new(bind_addr, port);
    let conf = Arc::new(Mutex::new(conf));
    let make_service = make_service_fn(move |_conn| {
        let conf = conf.clone();
        async move { Ok::<_, Infallible>(service_fn(move |req| handle(conf.clone(), req))) }
    });
    let server = Server::bind(&socket_addr).serve(make_service);
    let port = server.local_addr().port();
    (server, port)
}
