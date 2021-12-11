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

use crate::network::dispatch::{DispatchError, Handler, MessageContext, MessageSender, PeerId};
use crate::protos::network::{NetworkEcho, NetworkHeartbeat, NetworkMessage, NetworkMessageType};

use protobuf::Message;

// Implements a handler that handles NetworkEcho Messages
pub struct NetworkEchoHandler {
    node_id: String,
}

impl Handler for NetworkEchoHandler {
    type Source = PeerId;
    type MessageType = NetworkMessageType;
    type Message = NetworkEcho;

    fn match_type(&self) -> Self::MessageType {
        NetworkMessageType::NETWORK_ECHO
    }

    fn handle(
        &self,
        mut msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
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
            .send(recipient.into(), network_msg_bytes)
            .map_err(|(recipient, payload)| {
                DispatchError::NetworkSendError((recipient.into(), payload))
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
    type Source = PeerId;
    type MessageType = NetworkMessageType;
    type Message = NetworkHeartbeat;

    fn match_type(&self) -> Self::MessageType {
        NetworkMessageType::NETWORK_HEARTBEAT
    }

    fn handle(
        &self,
        _msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        _sender: &dyn MessageSender<Self::Source>,
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

    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use crate::network::dispatch::Dispatcher;
    use crate::protos::network::{NetworkEcho, NetworkMessageType};

    #[test]
    fn dispatch_to_handler() {
        let network_sender = MockSender::new();
        let mut dispatcher: Dispatcher<NetworkMessageType> =
            Dispatcher::new(Box::new(network_sender.clone()));

        let handler = NetworkEchoHandler::new("TestPeer".to_string());

        dispatcher.set_handler(Box::new(handler));

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
                "OTHER_PEER".into(),
                &NetworkMessageType::NETWORK_ECHO,
                outgoing_message_bytes.clone()
            )
        );

        let (_, network_message) = network_sender
            .next_outbound()
            .expect("Unable to get expected message");

        let network_msg: NetworkMessage = Message::parse_from_bytes(&network_message).unwrap();
        let echo: NetworkEcho = Message::parse_from_bytes(network_msg.get_payload()).unwrap();

        assert_eq!(echo.get_recipient(), "TestPeer");
        assert_eq!(echo.get_time_to_live(), 2);
        assert_eq!(echo.get_payload().to_vec(), b"HelloWorld".to_vec());
    }

    #[derive(Clone)]
    struct MockSender {
        outbound: Arc<Mutex<VecDeque<(PeerId, Vec<u8>)>>>,
    }

    impl MockSender {
        fn new() -> Self {
            Self {
                outbound: Arc::new(Mutex::new(VecDeque::new())),
            }
        }

        fn next_outbound(&self) -> Option<(PeerId, Vec<u8>)> {
            self.outbound.lock().expect("lock was poisoned").pop_front()
        }
    }

    impl MessageSender<PeerId> for MockSender {
        fn send(&self, id: PeerId, message: Vec<u8>) -> Result<(), (PeerId, Vec<u8>)> {
            self.outbound
                .lock()
                .expect("lock was poisoned")
                .push_back((id, message));

            Ok(())
        }
    }
}
