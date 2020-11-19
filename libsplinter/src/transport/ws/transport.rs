// Copyright 2018-2020 Cargill Incorporated
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::net::{Ipv4Addr, Ipv6Addr, TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use openssl::error::ErrorStack;
use openssl::ssl::{SslAcceptor, SslConnector};
use tungstenite::{client, handshake::HandshakeError};
use url::{ParseError, Url};

use crate::transport::tls::{build_acceptor, build_connector, TlsConfig};
use crate::transport::{ConnectError, Connection, ListenError, Listener, Transport};

use super::connection::WsConnection;
use super::listener::WsListener;

pub(super) const WS_PROTOCOL_PREFIX: &str = "ws://";
pub(super) const WSS_PROTOCOL_PREFIX: &str = "wss://";

struct TlsInner {
    acceptor: SslAcceptor,
    connector: SslConnector,
}

/// A WebSocket-based `Transport`.
///
/// Supports endpoints of the format `ws://ip_or_host:port`.
///
/// # Examples
///
/// To connect to the a remote endpoint, send a message, and receive a reply message:
///
/// ```rust,no_run
/// use splinter::transport::Transport as _;
/// use splinter::transport::ws::WsTransport;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut transport = WsTransport::new(None)?;
///
///     // Connect to a remote endpoint starting wtih `ws://`.
///     let mut connection = transport.connect("ws://127.0.0.1:5555")?;
///
///     // Send some bytes
///     connection.send(b"hello world")?;
///
///     // Receive a response
///     let msg = connection.recv()?;
///
///     // Disconnect
///     connection.disconnect()?;
///
///     Ok(())
/// }
/// ```
///
/// To accept a connection, receive, and send a reply:
///
/// ```rust, no_run
/// use splinter::transport::Transport as _;
/// use splinter::transport::ws::WsTransport;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut transport = WsTransport::new(None)?;
///
///     // Create a listener, which will bind to the port
///     let mut listener = transport.listen("ws://127.0.0.1:5555")?;
///
///     // When the other side connects, accept will return a `Connection`
///     let mut connection = listener.accept()?;
///
///     // Receive a message
///     let msg = connection.recv()?;
///
///     // Send a response
///     connection.send(b"hello world")?;
///
///     // Disconnect
///     connection.disconnect()?;
///
///     Ok(())
/// }
/// ```
#[derive(Default)]
pub struct WsTransport {
    tls_inner: Option<TlsInner>,
}

impl WsTransport {
    pub fn new(config: Option<&TlsConfig>) -> Result<Self, WsInitError> {
        if let Some(conf) = config {
            Ok(WsTransport {
                tls_inner: Some(TlsInner {
                    acceptor: build_acceptor(&conf)?,
                    connector: build_connector(&conf)?,
                }),
            })
        } else {
            Ok(WsTransport { tls_inner: None })
        }
    }
}

fn endpoint_to_dns_name(endpoint: &str) -> Result<String, ParseError> {
    let mut address = String::from("wss://");
    address.push_str(endpoint);
    let url = Url::parse(&address)?;
    let dns_name = match url.domain() {
        Some(d) if d.parse::<Ipv4Addr>().is_ok() => "localhost",
        Some(d) if d.parse::<Ipv6Addr>().is_ok() => "localhost",
        Some(d) => d,
        None => "localhost",
    };
    Ok(String::from(dns_name))
}

impl Transport for WsTransport {
    fn accepts(&self, address: &str) -> bool {
        address.starts_with(WS_PROTOCOL_PREFIX) || address.starts_with(WSS_PROTOCOL_PREFIX)
    }

    fn connect(&mut self, endpoint: &str) -> Result<Box<dyn Connection>, ConnectError> {
        if let Some(address) = endpoint.strip_prefix(WS_PROTOCOL_PREFIX) {
            let stream = TcpStream::connect(address)?;

            let remote_endpoint = format!("{}{}", WS_PROTOCOL_PREFIX, stream.peer_addr()?);
            let local_endpoint = format!("{}{}", WS_PROTOCOL_PREFIX, stream.local_addr()?);

            let (websocket, _) = client(endpoint, stream).map_or_else(
                {
                    |mut handshake_err| loop {
                        match handshake_err {
                            HandshakeError::Interrupted(mid_handshake) => {
                                thread::sleep(Duration::from_millis(100));
                                match mid_handshake.handshake() {
                                    Ok(ok) => break Ok(ok),
                                    Err(err) => handshake_err = err,
                                }
                            }
                            HandshakeError::Failure(err) => break Err(err),
                        }
                    }
                },
                Ok,
            )?;

            Ok(Box::new(WsConnection::new(
                websocket,
                remote_endpoint,
                local_endpoint,
            )))
        } else if let Some(address) = endpoint.strip_prefix(WSS_PROTOCOL_PREFIX) {
            let dns_name = endpoint_to_dns_name(address)?;

            let stream = TcpStream::connect(address)?;

            let remote_endpoint = format!("{}{}", WSS_PROTOCOL_PREFIX, stream.peer_addr()?);
            let local_endpoint = format!("{}{}", WSS_PROTOCOL_PREFIX, stream.local_addr()?);

            let tls_stream = self
                .tls_inner
                .as_ref()
                .ok_or_else(|| {
                    ConnectError::ProtocolError(format!(
                        "Protocol {} requires TLS, which is not configured",
                        WSS_PROTOCOL_PREFIX
                    ))
                })?
                .connector
                .connect(&dns_name, stream)?;

            let (websocket, _) = client(endpoint, tls_stream).map_or_else(
                {
                    |mut handshake_err| loop {
                        match handshake_err {
                            HandshakeError::Interrupted(mid_handshake) => {
                                thread::sleep(Duration::from_millis(100));
                                match mid_handshake.handshake() {
                                    Ok(ok) => break Ok(ok),
                                    Err(err) => handshake_err = err,
                                }
                            }
                            HandshakeError::Failure(err) => break Err(err),
                        }
                    }
                },
                Ok,
            )?;

            Ok(Box::new(WsConnection::new(
                websocket,
                remote_endpoint,
                local_endpoint,
            )))
        } else {
            Err(ConnectError::ProtocolError(format!(
                "Invalid protocol: {}",
                endpoint
            )))
        }
    }

    fn listen(&mut self, bind: &str) -> Result<Box<dyn Listener>, ListenError> {
        if let Some(address) = bind.strip_prefix(WS_PROTOCOL_PREFIX) {
            let tcp_listener = TcpListener::bind(address).map_err(|err| {
                ListenError::IoError(format!("Failed to bind to {}", address), err)
            })?;
            let local_endpoint = format!(
                "{}{}",
                WS_PROTOCOL_PREFIX,
                tcp_listener.local_addr().map_err(|err| {
                    ListenError::IoError("Failed to get local address".into(), err)
                })?
            );

            Ok(Box::new(WsListener::new(
                tcp_listener,
                local_endpoint,
                None,
            )))
        } else if let Some(address) = bind.strip_prefix(WSS_PROTOCOL_PREFIX) {
            let inner = self.tls_inner.as_ref().ok_or_else(|| {
                ListenError::ProtocolError(
                    "TLS support required for the wss:// protocol".to_string(),
                )
            })?;

            let tcp_listener = TcpListener::bind(address).map_err(|err| {
                ListenError::IoError(format!("Failed to bind to {}", address), err)
            })?;
            let local_endpoint = format!(
                "{}{}",
                WSS_PROTOCOL_PREFIX,
                tcp_listener.local_addr().map_err(|err| {
                    ListenError::IoError("Failed to get local address".into(), err)
                })?
            );

            Ok(Box::new(WsListener::new(
                tcp_listener,
                local_endpoint,
                Some(inner.acceptor.clone()),
            )))
        } else {
            Err(ListenError::ProtocolError(format!(
                "Invalid protocol: {}",
                bind
            )))
        }
    }
}

impl From<tungstenite::error::Error> for ConnectError {
    fn from(err: tungstenite::error::Error) -> Self {
        match err {
            tungstenite::error::Error::Io(io) => ConnectError::from(io),
            _ => ConnectError::ProtocolError(format!("handshake failure: {}", err)),
        }
    }
}

#[derive(Debug)]
pub enum WsInitError {
    ProtocolError(String),
}

impl std::error::Error for WsInitError {}

impl std::fmt::Display for WsInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            WsInitError::ProtocolError(msg) => write!(f, "Unable to initialize TLS: {}", msg),
        }
    }
}

impl From<ErrorStack> for WsInitError {
    fn from(error: ErrorStack) -> Self {
        WsInitError::ProtocolError(format!("OpenSSL error: {}", error))
    }
}
