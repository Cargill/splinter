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

use std::net::TcpStream;

use websocket::server::{sync::Server, upgrade::sync::Buffer, InvalidConnection, NoTlsAcceptor};

use crate::transport::{AcceptError, Connection, Listener};

use super::connection::WsClientConnection;

pub(super) struct WsListener {
    server: Server<NoTlsAcceptor>,
    local_endpoint: String,
}

impl WsListener {
    pub fn new(server: Server<NoTlsAcceptor>, local_endpoint: String) -> Self {
        WsListener {
            server,
            local_endpoint,
        }
    }
}

impl Listener for WsListener {
    fn accept(&mut self) -> Result<Box<dyn Connection>, AcceptError> {
        let client = self.server.accept()?.accept()?;

        let remote_endpoint = format!("ws://{}", client.peer_addr()?);
        let local_endpoint = format!("ws://{}", client.local_addr()?);

        Ok(Box::new(WsClientConnection::new(
            client,
            remote_endpoint,
            local_endpoint,
        )))
    }

    fn endpoint(&self) -> String {
        self.local_endpoint.clone()
    }
}

impl From<InvalidConnection<TcpStream, Buffer>> for AcceptError {
    fn from(iconn: InvalidConnection<TcpStream, Buffer>) -> Self {
        AcceptError::ProtocolError(format!("HyperIntoWsError: {}", iconn.error.to_string()))
    }
}

impl From<(TcpStream, std::io::Error)> for AcceptError {
    fn from(tuple: (TcpStream, std::io::Error)) -> Self {
        AcceptError::from(tuple.1)
    }
}
