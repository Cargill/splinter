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
use std::sync::mpsc::{channel, Sender};

use crate::network::Network;

// Message to send to the network message sender with the recipient and payload
#[derive(Clone, Debug, PartialEq)]
pub(in crate::network) enum SendRequest {
    Shutdown,
    Message { recipient: String, payload: Vec<u8> },
}

#[derive(Clone)]
pub struct NetworkMessageSender {
    sender: Sender<SendRequest>,
}

impl NetworkMessageSender {
    pub(in crate::network) fn new(sender: Sender<SendRequest>) -> Self {
        NetworkMessageSender { sender }
    }

    pub fn send(&self, recipient: String, payload: Vec<u8>) -> Result<(), (String, Vec<u8>)> {
        self.sender
            .send(SendRequest::Message { recipient, payload })
            .map_err(|err| match err.0 {
                SendRequest::Message { recipient, payload } => (recipient, payload),
                SendRequest::Shutdown => unreachable!(), // we didn't send this
            })
    }
}

pub struct ShutdownSignaler {
    sender: Sender<SendRequest>,
}

impl ShutdownSignaler {
    pub fn shutdown(&self) {
        if self.sender.send(SendRequest::Shutdown).is_err() {
            error!("Unable to send shutdown signal to already-shutdown network message queue");
        }
    }
}

#[derive(Default)]
pub struct Builder {
    network: Option<Network>,
}

impl Builder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_network(mut self, network: Network) -> Self {
        self.network = Some(network);
        self
    }

    pub fn build(mut self) -> Result<NetworkMessageSendQueue, String> {
        let (tx, rx) = channel();

        let network = self
            .network
            .take()
            .ok_or_else(|| "No network provided".to_string())?;

        let join_handle = std::thread::spawn(move || {
            loop {
                match rx.recv() {
                    Ok(SendRequest::Message { recipient, payload }) => {
                        if let Err(err) = network.send(&recipient, &payload) {
                            warn!("Unable to send message: {:?}", err);
                        }
                    }
                    Ok(SendRequest::Shutdown) => {
                        debug!("Received shutdown signal");
                        break;
                    }
                    Err(_) => {
                        debug!("No more network senders attached to this queue; aborting");
                        break;
                    }
                }
            }

            debug!("Exiting network message send queue loop");
        });

        Ok(NetworkMessageSendQueue {
            sender: tx,
            join_handle,
        })
    }
}

// The NetworkMessageSendQueue recv messages that should be sent over the network. The Sender side of
// the channel will be passed to handlers.
pub struct NetworkMessageSendQueue {
    sender: Sender<SendRequest>,
    join_handle: std::thread::JoinHandle<()>,
}

impl NetworkMessageSendQueue {
    pub fn wait_for_shutdown(self) {
        if self.join_handle.join().is_err() {
            error!("Unable to cleanly wait for network message send queue shutdown");
        }
    }

    pub fn new_network_sender(&self) -> NetworkMessageSender {
        NetworkMessageSender {
            sender: self.sender.clone(),
        }
    }

    pub fn shutdown_signaler(&self) -> ShutdownSignaler {
        ShutdownSignaler {
            sender: self.sender.clone(),
        }
    }
}

#[derive(Debug)]
pub enum NetworkMessageSenderError {
    RecvTimeoutError(String),
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;
    use crate::mesh::Mesh;
    use crate::network::Network;
    use crate::transport::inproc::InprocTransport;
    use crate::transport::Transport;

    // Test that a message can successfully be sent by passing it to the
    // NetworkMessageSender, the message is received by the NetworkMessageSendQueue, and then
    // sent over the network.
    #[test]
    fn test_network_message_sender() {
        let mut transport = InprocTransport::default();
        let mut listener = transport.listen("inproc://sender").unwrap();
        let endpoint = listener.endpoint();

        let mesh1 = Mesh::new(1, 1);
        let network1 = Network::new(mesh1.clone(), 0).unwrap();

        let network_message_queue = Builder::new()
            .with_network(network1.clone())
            .build()
            .expect("Unable to create queue");
        let network_message_sender = network_message_queue.new_network_sender();

        thread::spawn(move || {
            let mesh2 = Mesh::new(1, 1);
            let network2 = Network::new(mesh2.clone(), 0).unwrap();
            let connection = listener.accept().unwrap();
            network2.add_peer("ABC".to_string(), connection).unwrap();
            let network_message = network2.recv().unwrap();
            assert_eq!(network_message.peer_id(), "ABC".to_string());
            assert_eq!(
                network_message.payload().to_vec(),
                b"FromNetworkMessageSender".to_vec()
            );

            network2
                .send("ABC", &vec![])
                .expect("Unable to send message");
        });

        let connection = transport.connect(&endpoint).unwrap();
        network1.add_peer("123".to_string(), connection).unwrap();

        network_message_sender
            .send("123".to_string(), b"FromNetworkMessageSender".to_vec())
            .unwrap();

        // block until we can receive a message from the other thread
        network1.recv().expect("Unable to recv message");
    }

    // Test that 100 messages can successfully be sent by passing them to the
    // NetworkMessageSender, the messages are received by the NetworkMessageSendQueue, and then
    // sent over the network.
    #[test]
    fn test_network_message_sender_rapid_fire() {
        let mut transport = InprocTransport::default();
        let mut listener = transport.listen("inproc://sender").unwrap();

        let mesh1 = Mesh::new(5, 5);
        let network1 = Network::new(mesh1.clone(), 0).unwrap();

        let network_message_queue = Builder::new()
            .with_network(network1.clone())
            .build()
            .expect("Unable to create queue");
        let network_message_sender = network_message_queue.new_network_sender();

        thread::spawn(move || {
            let mesh2 = Mesh::new(5, 5);
            let network2 = Network::new(mesh2.clone(), 0).unwrap();
            let connection = listener.accept().unwrap();
            network2.add_peer("ABC".to_string(), connection).unwrap();
            for _ in 0..100 {
                let network_message = network2.recv().unwrap();
                assert_eq!(network_message.peer_id(), "ABC".to_string());
                assert_eq!(
                    network_message.payload().to_vec(),
                    b"FromNetworkMessageSender".to_vec()
                );
                network2
                    .send("ABC", &vec![])
                    .expect("Unable to send message");
            }
        });

        let connection = transport.connect("inproc://sender").unwrap();
        network1.add_peer("123".to_string(), connection).unwrap();
        for _ in 0..100 {
            network_message_sender
                .send("123".to_string(), b"FromNetworkMessageSender".to_vec())
                .expect("Unable to send message");

            // block until we can receive a message from the other thread
            network1.recv().expect("Unable to recv message");
        }
    }
}
