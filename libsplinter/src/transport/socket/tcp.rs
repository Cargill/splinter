// Copyright 2018-2021 Cargill Incorporated
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

use mio::{net::TcpStream as MioTcpStream, Evented};

use std::net::{Shutdown, TcpListener as StdTcpListener, TcpStream};

use crate::transport::{
    AcceptError, ConnectError, Connection, DisconnectError, ListenError, Listener, RecvError,
    SendError, Transport,
};

use super::frame::{Frame, FrameError, FrameNegotiation, FrameRef, FrameVersion};

const PROTOCOL_PREFIX: &str = "tcp://";

#[derive(Default)]
pub struct TcpTransport {}

impl Transport for TcpTransport {
    fn accepts(&self, address: &str) -> bool {
        address.starts_with(PROTOCOL_PREFIX) || !address.contains("://")
    }

    fn connect(&mut self, endpoint: &str) -> Result<Box<dyn Connection>, ConnectError> {
        if !self.accepts(endpoint) {
            return Err(ConnectError::ProtocolError(format!(
                "Invalid protocol \"{}\"",
                endpoint
            )));
        }

        let address = if let Some(address) = endpoint.strip_prefix(PROTOCOL_PREFIX) {
            address
        } else {
            endpoint
        };
        // Connect a std::net::TcpStream to make sure connect() block
        let mut stream = TcpStream::connect(address)?;

        let frame_version = FrameNegotiation::outbound(FrameVersion::V1, FrameVersion::V1)
            .negotiate(&mut stream)
            .map_err(|err| match err {
                FrameError::UnsupportedVersion => ConnectError::ProtocolError(
                    "Unable to connect; remote version is not with in range".into(),
                ),
                FrameError::IoError(err) => ConnectError::from(err),
                e => ConnectError::ProtocolError(format!("Unexpected protocol error: {}", e)),
            })?;

        let mio_stream = MioTcpStream::from_stream(stream)?;
        Ok(Box::new(TcpConnection {
            frame_version,
            stream: mio_stream,
        }))
    }

    fn listen(&mut self, bind: &str) -> Result<Box<dyn Listener>, ListenError> {
        if !self.accepts(bind) {
            return Err(ListenError::ProtocolError(format!(
                "Invalid protocol \"{}\"",
                bind
            )));
        }

        let address = if let Some(address) = bind.strip_prefix(PROTOCOL_PREFIX) {
            address
        } else {
            bind
        };

        Ok(Box::new(TcpListener {
            listener: StdTcpListener::bind(address).map_err(|err| {
                ListenError::IoError(format!("Failed to bind to {}", address), err)
            })?,
        }))
    }
}

struct TcpListener {
    listener: StdTcpListener,
}

impl Listener for TcpListener {
    fn accept(&mut self) -> Result<Box<dyn Connection>, AcceptError> {
        let (mut stream, _) = self.listener.accept()?;

        let frame_version = FrameNegotiation::inbound(FrameVersion::V1)
            .negotiate(&mut stream)
            .map_err(|err| match err {
                FrameError::UnsupportedVersion => AcceptError::ProtocolError(format!(
                    "Local {} protocol version {} not supported by remote",
                    PROTOCOL_PREFIX,
                    FrameVersion::V1
                )),
                FrameError::IoError(err) => AcceptError::from(err),
                err => AcceptError::ProtocolError(format!("Unexpected protocol error: {}", err)),
            })?;

        let connection = TcpConnection {
            frame_version,
            stream: MioTcpStream::from_stream(stream)?,
        };
        Ok(Box::new(connection))
    }

    fn endpoint(&self) -> String {
        format!("tcp://{}", self.listener.local_addr().unwrap())
    }
}

struct TcpConnection {
    frame_version: FrameVersion,
    stream: MioTcpStream,
}

impl Connection for TcpConnection {
    fn send(&mut self, message: &[u8]) -> Result<(), SendError> {
        match FrameRef::new(self.frame_version, message).write(&mut self.stream) {
            Err(FrameError::IoError(e)) => Err(SendError::from(e)),
            Err(err) => Err(SendError::ProtocolError(err.to_string())),
            Ok(_) => Ok(()),
        }
    }

    fn recv(&mut self) -> Result<Vec<u8>, RecvError> {
        match Frame::read(&mut self.stream) {
            Err(FrameError::IoError(e)) => Err(RecvError::from(e)),
            Err(err) => Err(RecvError::ProtocolError(err.to_string())),
            Ok(frame) => Ok(frame.into_inner()),
        }
    }

    fn remote_endpoint(&self) -> String {
        format!("tcp://{}", self.stream.peer_addr().unwrap())
    }

    fn local_endpoint(&self) -> String {
        format!("tcp://{}", self.stream.local_addr().unwrap())
    }

    fn disconnect(&mut self) -> Result<(), DisconnectError> {
        self.stream
            .shutdown(Shutdown::Both)
            .map_err(DisconnectError::from)
    }

    fn evented(&self) -> &dyn Evented {
        &self.stream
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::tests;

    #[test]
    fn test_accepts() {
        let transport = TcpTransport::default();
        assert!(transport.accepts("127.0.0.1:0"));
        assert!(transport.accepts("tcp://127.0.0.1:0"));
        assert!(transport.accepts("tcp://somewhere.example.com:4000"));

        assert!(!transport.accepts("tls://somewhere.example.com:4000"));
        assert!(!transport.accepts("tcps://somewhere.example.com:4000"));
    }

    #[test]
    fn test_transport() {
        let transport = TcpTransport::default();

        tests::test_transport(transport, "127.0.0.1:0");
    }

    #[test]
    fn test_transport_explicit_protocol() {
        let transport = TcpTransport::default();

        tests::test_transport(transport, "tcp://127.0.0.1:0");
    }

    #[test]
    fn test_poll() {
        let transport = TcpTransport::default();
        tests::test_poll(transport, "127.0.0.1:0");
    }
}
