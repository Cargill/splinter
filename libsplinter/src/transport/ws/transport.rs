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

use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use mio::net::TcpStream as MioTcpStream;
use tungstenite::{client, handshake::HandshakeError};

use crate::transport::{ConnectError, Connection, ListenError, Listener, Transport};

use super::connection::WsConnection;
use super::listener::WsListener;

pub(super) const PROTOCOL_PREFIX: &str = "ws://";

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
///     let mut transport = WsTransport::default();
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
///     let mut transport = WsTransport::default();
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
pub struct WsTransport {}

impl Transport for WsTransport {
    fn accepts(&self, address: &str) -> bool {
        address.starts_with(PROTOCOL_PREFIX)
    }

    fn connect(&mut self, endpoint: &str) -> Result<Box<dyn Connection>, ConnectError> {
        if !self.accepts(endpoint) {
            return Err(ConnectError::ProtocolError(format!(
                "Invalid protocol \"{}\"",
                endpoint
            )));
        }

        let address = if endpoint.starts_with(PROTOCOL_PREFIX) {
            &endpoint[PROTOCOL_PREFIX.len()..]
        } else {
            endpoint
        };

        let stream = TcpStream::connect(address)?;

        let remote_endpoint = format!("{}{}", PROTOCOL_PREFIX, stream.peer_addr()?);
        let local_endpoint = format!("{}{}", PROTOCOL_PREFIX, stream.local_addr()?);

        let mio_stream = MioTcpStream::from_stream(stream)?;

        let (websocket, _) = client(endpoint, mio_stream).map_or_else(
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
    }

    fn listen(&mut self, bind: &str) -> Result<Box<dyn Listener>, ListenError> {
        if !self.accepts(bind) {
            return Err(ListenError::ProtocolError(format!(
                "Invalid protocol \"{}\"",
                bind
            )));
        }

        let address = if bind.starts_with(PROTOCOL_PREFIX) {
            &bind[PROTOCOL_PREFIX.len()..]
        } else {
            bind
        };

        let tcp_listener = TcpListener::bind(address)?;
        let local_endpoint = format!("ws://{}", tcp_listener.local_addr()?);

        Ok(Box::new(WsListener::new(tcp_listener, local_endpoint)))
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
