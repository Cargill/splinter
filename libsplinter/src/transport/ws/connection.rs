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
use std::net::TcpStream;
use std::os::unix::io::{AsRawFd, RawFd};

use mio::{unix::EventedFd, Evented, Poll, PollOpt, Ready, Token};
use openssl::ssl::SslStream;
use tungstenite::{protocol::WebSocket, Message};

use crate::transport::{Connection, DisconnectError, RecvError, SendError};

pub(super) struct WsConnection<S>
where
    S: Read + Write + Send,
{
    websocket: WebSocket<S>,
    remote_endpoint: String,
    local_endpoint: String,
}

impl<S> WsConnection<S>
where
    S: Read + Write + Send,
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
    S: Read + Write + Send + WsAsRawFd,
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
        self
    }
}

pub(super) trait WsAsRawFd {
    fn ws_as_raw_fd(&self) -> RawFd;
}

impl WsAsRawFd for TcpStream {
    fn ws_as_raw_fd(&self) -> RawFd {
        self.as_raw_fd()
    }
}

impl WsAsRawFd for SslStream<TcpStream> {
    fn ws_as_raw_fd(&self) -> RawFd {
        self.get_ref().as_raw_fd()
    }
}

impl<S> AsRawFd for WsConnection<S>
where
    S: Read + Write + Send + WsAsRawFd,
{
    fn as_raw_fd(&self) -> RawFd {
        self.websocket.get_ref().ws_as_raw_fd()
    }
}

impl<S> Evented for WsConnection<S>
where
    S: Read + Write + Send + WsAsRawFd,
{
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> std::io::Result<()> {
        EventedFd(&self.as_raw_fd()).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> std::io::Result<()> {
        EventedFd(&self.as_raw_fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> std::io::Result<()> {
        EventedFd(&self.as_raw_fd()).deregister(poll)
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
