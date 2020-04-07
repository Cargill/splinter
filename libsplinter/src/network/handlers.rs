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

use crate::network::dispatch::{DispatchError, Handler, MessageContext};
use crate::network::sender::NetworkMessageSender;
use crate::protos::network::{NetworkEcho, NetworkHeartbeat, NetworkMessage, NetworkMessageType};

use protobuf::Message;

// Implements a handler that handles NetworkEcho Messages
pub struct NetworkEchoHandler {
    node_id: String,
}

impl Handler for NetworkEchoHandler {
    type MessageType = NetworkMessageType;
    type Message = NetworkEcho;

    fn handle(
        &self,
        mut msg: Self::Message,
        context: &MessageContext<Self::MessageType>,
        sender: &NetworkMessageSender,
    ) -> Result<(), DispatchError> {
        debug!("ECHO: {:?}", msg);

        let recipient = {
            // if the recipient is us forward back to sender else forward on to the intended
            // recipient
            if msg.get_recipient() == self.node_id {
                context.source_peer_id().to_string()
            } else {
                msg.get_recipient().to_string()
            }
        };

        msg.set_time_to_live(msg.get_time_to_live() - 1);
        if msg.get_time_to_live() <= 0 {
            return Ok(());
        };

        let echo_bytes = msg.write_to_bytes().unwrap();

        let mut network_msg = NetworkMessage::new();
        network_msg.set_message_type(NetworkMessageType::NETWORK_ECHO);
        network_msg.set_payload(echo_bytes);
        let network_msg_bytes = network_msg.write_to_bytes().unwrap();

        sender
            .send(recipient, network_msg_bytes)
            .map_err(|(recipient, payload)| {
                DispatchError::NetworkSendError((recipient, payload))
            })?;
        Ok(())
    }
}

impl NetworkEchoHandler {
    pub fn new(node_id: String) -> Self {
        NetworkEchoHandler { node_id }
    }
}

// Implements a handler that handles NetworkHeartbeat Messages
#[derive(Default)]
pub struct NetworkHeartbeatHandler {}

impl Handler for NetworkHeartbeatHandler {
    type MessageType = NetworkMessageType;
    type Message = NetworkHeartbeat;

    fn handle(
        &self,
        _msg: Self::Message,
        context: &MessageContext<Self::MessageType>,
        _sender: &NetworkMessageSender,
    ) -> Result<(), DispatchError> {
        trace!("Received Heartbeat from {}", context.source_peer_id());
        Ok(())
    }
}

impl NetworkHeartbeatHandler {
    pub fn new() -> Self {
        NetworkHeartbeatHandler {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Mesh;
    use crate::network::dispatch::Dispatcher;
    use crate::network::sender;
    use crate::network::Network;
    use crate::protos::network::{NetworkEcho, NetworkMessageType};
    use crate::transport::inproc::InprocTransport;
    use crate::transport::Transport;

    #[test]
    fn dispatch_to_handler() {
        // Set up dispatcher and mock sender
        let mesh1 = Mesh::new(1, 1);
        let network1 = Network::new(mesh1.clone(), 0).unwrap();

        let network_message_queue = sender::Builder::new()
            .with_network(network1.clone())
            .build()
            .expect("Unable to create queue");
        let network_sender = network_message_queue.new_network_sender();

        let mut inproc_transport = InprocTransport::default();
        let mut dispatcher: Dispatcher<NetworkMessageType> = Dispatcher::new(network_sender);
        let mut listener = inproc_transport
            .listen("inproc://network_echo")
            .expect("Cannot get listener");

        std::thread::spawn(move || {
            let connection = listener.accept().expect("Cannot accept connection");
            network1
                .add_peer("OTHER_PEER".to_string(), connection)
                .expect("Unable to add peer");

            let handler = NetworkEchoHandler::new("TestPeer".to_string());

            dispatcher.set_handler(NetworkMessageType::NETWORK_ECHO, Box::new(handler));

            let msg = {
                let mut echo = NetworkEcho::new();
                echo.set_payload(b"HelloWorld".to_vec());
                echo.set_recipient("TestPeer".to_string());
                echo.set_time_to_live(3);
                echo
            };

            let outgoing_message_bytes = msg.write_to_bytes().unwrap();

            assert_eq!(
                Ok(()),
                dispatcher.dispatch(
                    "OTHER_PEER",
                    &NetworkMessageType::NETWORK_ECHO,
                    outgoing_message_bytes.clone()
                )
            );
        });

        let mesh2 = Mesh::new(1, 1);
        let network2 = Network::new(mesh2.clone(), 0).unwrap();
        let connection = inproc_transport
            .connect("inproc://network_echo")
            .expect("Unable to connect to inproc");
        network2
            .add_peer("TestPeer".to_string(), connection)
            .expect("Unable to add peer");
        let network_message = network2
            .recv()
            .expect("Unable to receive message over the network");
        let network_msg: NetworkMessage =
            protobuf::parse_from_bytes(network_message.payload()).unwrap();
        let echo: NetworkEcho = protobuf::parse_from_bytes(network_msg.get_payload()).unwrap();

        assert_eq!(echo.get_recipient(), "TestPeer");
        assert_eq!(echo.get_time_to_live(), 2);
        assert_eq!(echo.get_payload().to_vec(), b"HelloWorld".to_vec());
    }
}
