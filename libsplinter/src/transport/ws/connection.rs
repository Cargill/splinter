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

use std::io::{Read, Write};

use mio::Evented;
use tungstenite::{protocol::WebSocket, Message};

use crate::transport::{Connection, DisconnectError, RecvError, SendError};

pub(super) struct WsConnection<S>
where
    S: Read + Write + Send + Evented,
{
    websocket: WebSocket<S>,
    remote_endpoint: String,
    local_endpoint: String,
}

impl<S> WsConnection<S>
where
    S: Read + Write + Send + Evented,
{
    pub fn new(websocket: WebSocket<S>, remote_endpoint: String, local_endpoint: String) -> Self {
        WsConnection {
            websocket,
            remote_endpoint,
            local_endpoint,
        }
    }
}

impl<S> Connection for WsConnection<S>
where
    S: Read + Write + Send + Evented,
{
    fn send(&mut self, message: &[u8]) -> Result<(), SendError> {
        self.websocket
            .write_message(Message::Binary(message.to_vec()))?;
        self.websocket.write_pending()?;
        Ok(())
    }

    fn recv(&mut self) -> Result<Vec<u8>, RecvError> {
        match self.websocket.read_message() {
            Ok(message) => match message {
                Message::Binary(v) => Ok(v),
                _ => Err(RecvError::ProtocolError(
                    "message received was not binary".to_string(),
                )),
            },
            Err(tungstenite::error::Error::Io(e)) => Err(RecvError::from(e)),
            Err(err) => Err(err.into()),
        }
    }

    fn remote_endpoint(&self) -> String {
        self.remote_endpoint.clone()
    }

    fn local_endpoint(&self) -> String {
        self.local_endpoint.clone()
    }

    fn disconnect(&mut self) -> Result<(), DisconnectError> {
        self.websocket.close(None)?;
        Ok(())
    }

    fn evented(&self) -> &dyn Evented {
        self.websocket.get_ref()
    }
}

impl From<tungstenite::error::Error> for SendError {
    fn from(err: tungstenite::error::Error) -> Self {
        match err {
            tungstenite::error::Error::Io(io) => SendError::from(io),
            _ => SendError::ProtocolError(err.to_string()),
        }
    }
}

impl From<tungstenite::error::Error> for RecvError {
    fn from(err: tungstenite::error::Error) -> Self {
        match err {
            tungstenite::error::Error::Io(io) => RecvError::from(io),
            _ => RecvError::ProtocolError(err.to_string()),
        }
    }
}

impl From<tungstenite::error::Error> for DisconnectError {
    fn from(err: tungstenite::error::Error) -> Self {
        match err {
            tungstenite::error::Error::Io(io) => DisconnectError::from(io),
            _ => DisconnectError::ProtocolError(err.to_string()),
        }
    }
}
