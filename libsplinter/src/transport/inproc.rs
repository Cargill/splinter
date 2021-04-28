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

use mio::Evented;
use mio_extras::channel as mio_channel;

use std::collections::HashMap;
use std::io::{self, ErrorKind};
use std::sync::{
    mpsc::{channel, Receiver, Sender},
    Arc, Mutex,
};

use crate::transport::{
    AcceptError, ConnectError, Connection, DisconnectError, ListenError, Listener, RecvError,
    SendError, Transport,
};

type Incoming = Arc<Mutex<HashMap<String, Sender<Pair<Vec<u8>>>>>>;

const PROTOCOL_PREFIX: &str = "inproc://";

#[derive(Clone, Default)]
pub struct InprocTransport {
    incoming: Incoming,
}

impl Transport for InprocTransport {
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

        match self.incoming.lock().unwrap().get(address) {
            Some(sender) => {
                let (p0, p1) = Pair::new();
                sender.send(p0).unwrap();
                Ok(Box::new(InprocConnection::new(address.into(), p1)))
            }
            None => Err(ConnectError::IoError(io::Error::new(
                ErrorKind::ConnectionRefused,
                format!("No InprocListener for {}", endpoint),
            ))),
        }
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

        let (tx, rx) = channel();
        self.incoming.lock().unwrap().insert(address.into(), tx);
        Ok(Box::new(InprocListener::new(address.into(), rx)))
    }
}

pub struct InprocListener {
    endpoint: String,
    rx: Receiver<Pair<Vec<u8>>>,
}

impl InprocListener {
    fn new(endpoint: String, rx: Receiver<Pair<Vec<u8>>>) -> Self {
        InprocListener { endpoint, rx }
    }
}

impl Listener for InprocListener {
    fn accept(&mut self) -> Result<Box<dyn Connection>, AcceptError> {
        Ok(Box::new(InprocConnection::new(
            self.endpoint.clone(),
            self.rx.recv().unwrap(),
        )))
    }

    fn endpoint(&self) -> String {
        let mut buf = String::from(PROTOCOL_PREFIX);
        buf.push_str(&self.endpoint);
        buf
    }
}

pub struct InprocConnection {
    endpoint: String,
    pair: Pair<Vec<u8>>,
}

impl InprocConnection {
    fn new(endpoint: String, pair: Pair<Vec<u8>>) -> Self {
        InprocConnection { endpoint, pair }
    }
}

impl Connection for InprocConnection {
    fn send(&mut self, message: &[u8]) -> Result<(), SendError> {
        self.pair.send(message.to_vec());
        Ok(())
    }

    fn recv(&mut self) -> Result<Vec<u8>, RecvError> {
        match self.pair.recv() {
            Some(message) => Ok(message),
            None => Err(RecvError::WouldBlock),
        }
    }

    fn remote_endpoint(&self) -> String {
        let mut buf = String::from(PROTOCOL_PREFIX);
        buf.push_str(&self.endpoint);
        buf
    }

    fn local_endpoint(&self) -> String {
        let mut buf = String::from(PROTOCOL_PREFIX);
        buf.push_str(&self.endpoint);
        buf
    }

    fn disconnect(&mut self) -> Result<(), DisconnectError> {
        Ok(())
    }

    fn evented(&self) -> &dyn Evented {
        &self.pair.incoming
    }
}

struct Pair<T> {
    outgoing: mio_channel::Sender<T>,
    incoming: mio_channel::Receiver<T>,
}

impl<T> Pair<T> {
    fn new() -> (Self, Self) {
        let (tx1, rx1) = mio_channel::channel();
        let (tx2, rx2) = mio_channel::channel();

        (
            Pair {
                outgoing: tx1,
                incoming: rx2,
            },
            Pair {
                outgoing: tx2,
                incoming: rx1,
            },
        )
    }

    fn send(&self, t: T) {
        self.outgoing.send(t).ok();
    }

    fn recv(&self) -> Option<T> {
        self.incoming.try_recv().ok()
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::transport::tests;

    #[test]
    fn test_transport() {
        let transport = InprocTransport::default();
        tests::test_transport(transport, "test");
    }

    #[cfg(not(unix))]
    #[test]
    fn test_poll() {
        let transport = InprocTransport::default();
        tests::test_poll(transport, "test");
    }
}
