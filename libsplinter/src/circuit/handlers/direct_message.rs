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

use crate::circuit::handlers::create_message;
use crate::circuit::{ServiceId, SplinterState};
use crate::network::dispatch::{DispatchError, Handler, MessageContext, MessageSender, PeerId};
use crate::protos::circuit::{
    CircuitDirectMessage, CircuitError, CircuitError_Error, CircuitMessageType,
};

use protobuf::Message;

// Implements a handler that handles CircuitDirectMessage
pub struct CircuitDirectMessageHandler {
    node_id: String,
    state: SplinterState,
}

impl Handler for CircuitDirectMessageHandler {
    type Source = PeerId;
    type MessageType = CircuitMessageType;
    type Message = CircuitDirectMessage;

    fn match_type(&self) -> Self::MessageType {
        CircuitMessageType::CIRCUIT_DIRECT_MESSAGE
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        debug!(
            "Handle Circuit Direct Message {}on {} ({} => {}) [{} byte{}]",
            if msg.get_correlation_id().is_empty() {
                "".to_string()
            } else {
                format!("{} ", msg.get_correlation_id())
            },
            msg.get_circuit(),
            msg.get_sender(),
            msg.get_recipient(),
            msg.get_payload().len(),
            if msg.get_payload().len() == 1 {
                ""
            } else {
                "s"
            }
        );

        let circuit_name = msg.get_circuit();
        let msg_sender = msg.get_sender();
        let recipient = msg.get_recipient();
        let recipient_id = ServiceId::new(circuit_name.to_string(), recipient.to_string());
        let sender_id = ServiceId::new(circuit_name.to_string(), msg_sender.to_string());

        // msg bytes will either be message bytes of a direct message or an error message
        // the msg_recipient is either the service/node id to send the message to or is the
        // peer_id to send back the error message
        let (msg_bytes, msg_recipient) = {
            if let Some(circuit) = self
                .state
                .circuit(circuit_name)
                .map_err(|err| DispatchError::HandleError(err.context()))?
            {
                // Check if the message sender is allowed on the circuit
                // if the sender is not allowed on the circuit
                if !circuit.roster().contains(&msg_sender) {
                    let mut error_message = CircuitError::new();
                    error_message.set_correlation_id(msg.get_correlation_id().to_string());
                    error_message.set_service_id(msg_sender.into());
                    error_message.set_circuit_name(circuit_name.into());
                    error_message.set_error(CircuitError_Error::ERROR_SENDER_NOT_IN_CIRCUIT_ROSTER);
                    error_message.set_error_message(format!(
                        "Sender is not allowed in the Circuit: {}",
                        msg_sender
                    ));

                    let msg_bytes = error_message.write_to_bytes()?;
                    let network_msg_bytes =
                        create_message(msg_bytes, CircuitMessageType::CIRCUIT_ERROR_MESSAGE)?;
                    (network_msg_bytes, context.source_peer_id().to_string())
                } else if self
                    .state
                    .get_service(&sender_id)
                    .map_err(|err| DispatchError::HandleError(err.context()))?
                    .is_none()
                {
                    // Check if the message sender is registered on the circuit
                    // if the sender is not connected, send circuit error
                    let mut error_message = CircuitError::new();
                    error_message.set_correlation_id(msg.get_correlation_id().to_string());
                    error_message.set_service_id(msg_sender.into());
                    error_message.set_circuit_name(circuit_name.into());
                    error_message.set_error(CircuitError_Error::ERROR_SENDER_NOT_IN_DIRECTORY);
                    error_message.set_error_message(format!(
                        "Sender is not in the service directory: {}",
                        recipient
                    ));

                    let msg_bytes = error_message.write_to_bytes()?;
                    let network_msg_bytes =
                        create_message(msg_bytes, CircuitMessageType::CIRCUIT_ERROR_MESSAGE)?;
                    (network_msg_bytes, context.source_peer_id().to_string())
                } else if circuit.roster().contains(&recipient) {
                    // check if the recipient service is allowed on the circuit and registered
                    if let Some(service) = self
                        .state
                        .get_service(&recipient_id)
                        .map_err(|err| DispatchError::HandleError(err.context()))?
                    {
                        let node_id = service.node().id().to_string();
                        // If the service is on this node send message to the service, otherwise
                        // send the message to the node the service is connected to
                        if node_id != self.node_id {
                            let msg_bytes = context.message_bytes().to_vec();
                            let network_msg_bytes = create_message(
                                msg_bytes,
                                CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
                            )?;
                            (network_msg_bytes, node_id)
                        } else {
                            let msg_bytes = context.message_bytes().to_vec();
                            let network_msg_bytes = create_message(
                                msg_bytes,
                                CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
                            )?;
                            let peer_id = match service.peer_id() {
                                Some(peer_id) => peer_id.clone(),
                                None => {
                                    // This should never happen, as a peer id will always
                                    // be set on a service that is connected to the local node.
                                    warn!("No peer id for service:{} ", service.service_id());
                                    return Ok(());
                                }
                            };
                            (network_msg_bytes, peer_id)
                        }
                    } else {
                        // This should not happen as every service should be added on circuit
                        // creation. If the recipient is not connected, send circuit error
                        let mut error_message = CircuitError::new();
                        error_message.set_correlation_id(msg.get_correlation_id().to_string());
                        error_message.set_service_id(msg_sender.into());
                        error_message.set_circuit_name(circuit_name.into());
                        error_message
                            .set_error(CircuitError_Error::ERROR_RECIPIENT_NOT_IN_DIRECTORY);
                        error_message.set_error_message(format!(
                            "Recipient is not in the service directory: {}",
                            recipient
                        ));

                        let msg_bytes = error_message.write_to_bytes()?;
                        let network_msg_bytes =
                            create_message(msg_bytes, CircuitMessageType::CIRCUIT_ERROR_MESSAGE)?;
                        (network_msg_bytes, context.source_peer_id().to_string())
                    }
                } else {
                    // if the recipient is not allowed on the circuit, send circuit error
                    let mut error_message = CircuitError::new();
                    error_message.set_correlation_id(msg.get_correlation_id().to_string());
                    error_message.set_service_id(msg_sender.into());
                    error_message.set_circuit_name(circuit_name.into());
                    error_message
                        .set_error(CircuitError_Error::ERROR_RECIPIENT_NOT_IN_CIRCUIT_ROSTER);
                    error_message.set_error_message(format!(
                        "Recipient is not allowed in the Circuit: {}",
                        recipient
                    ));

                    let msg_bytes = error_message.write_to_bytes()?;
                    let network_msg_bytes =
                        create_message(msg_bytes, CircuitMessageType::CIRCUIT_ERROR_MESSAGE)?;
                    (network_msg_bytes, context.source_peer_id().to_string())
                }
            } else {
                // if the circuit does not exist, send circuit error
                let mut error_message = CircuitError::new();
                error_message.set_correlation_id(msg.get_correlation_id().into());
                error_message.set_service_id(msg_sender.into());
                error_message.set_circuit_name(circuit_name.into());
                error_message.set_error(CircuitError_Error::ERROR_CIRCUIT_DOES_NOT_EXIST);
                error_message
                    .set_error_message(format!("Circuit does not exist: {}", circuit_name));

                let msg_bytes = error_message.write_to_bytes()?;
                let network_msg_bytes =
                    create_message(msg_bytes, CircuitMessageType::CIRCUIT_ERROR_MESSAGE)?;
                (network_msg_bytes, context.source_peer_id().to_string())
            }
        };

        // either forward the direct message or send back an error message.
        sender
            .send(msg_recipient.into(), msg_bytes)
            .map_err(|(recipient, payload)| {
                DispatchError::NetworkSendError((recipient.into(), payload))
            })?;
        Ok(())
    }
}

impl CircuitDirectMessageHandler {
    pub fn new(node_id: String, state: SplinterState) -> Self {
        CircuitDirectMessageHandler { node_id, state }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use crate::circuit::directory::CircuitDirectory;
    use crate::circuit::service::{Service, SplinterNode};
    use crate::circuit::{AuthorizationType, Circuit, DurabilityType, PersistenceType, RouteType};
    use crate::network::dispatch::Dispatcher;
    use crate::protos::circuit::CircuitMessage;
    use crate::protos::network::NetworkMessage;

    // Test that a direct message will be properly sent to the service if the message is meant for
    // a service connected to the receiving node
    #[test]
    fn test_circuit_direct_message_handler_service() {
        // Set up dispatcher and mock sender
        let mock_sender = MockSender::new();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));

        // Add circuit and service to splinter state
        let circuit = Circuit::builder()
            .with_id("alpha".into())
            .with_auth(AuthorizationType::Trust)
            .with_members(vec!["123".into()])
            .with_roster(vec!["abc".into(), "def".into()])
            .with_persistence(PersistenceType::Any)
            .with_durability(DurabilityType::NoDurability)
            .with_routes(RouteType::Any)
            .with_circuit_management_type("circuit_direct_test_app".into())
            .build()
            .expect("Should have built a correct circuit");

        let mut circuit_directory = CircuitDirectory::new();
        circuit_directory.add_circuit("alpha".to_string(), circuit);

        let state = SplinterState::new("memory".to_string(), circuit_directory);

        let node = SplinterNode::new("123".to_string(), vec!["123.0.0.1:0".to_string()]);
        let service_abc = Service::new(
            "abc".to_string(),
            Some("abc_network".to_string()),
            node.clone(),
        );
        let service_def = Service::new("def".to_string(), Some("def_network".to_string()), node);
        let abc_id = ServiceId::new("alpha".into(), "abc".into());
        let def_id = ServiceId::new("alpha".into(), "def".into());
        state.add_service(abc_id, service_abc).unwrap();
        state.add_service(def_id, service_def).unwrap();

        // Add direct message handler to the the dispatcher
        let handler = CircuitDirectMessageHandler::new("123".to_string(), state);
        dispatcher.set_handler(Box::new(handler));

        // Create the direct message
        let mut direct_message = CircuitDirectMessage::new();
        direct_message.set_circuit("alpha".into());
        direct_message.set_sender("def".into());
        direct_message.set_recipient("abc".into());
        direct_message.set_payload(b"test".to_vec());
        direct_message.set_correlation_id("1234".into());
        let direct_bytes = direct_message.write_to_bytes().unwrap();

        // dispatch the direct message
        dispatcher
            .dispatch(
                "def".into(),
                &CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
                direct_bytes.clone(),
            )
            .unwrap();

        let (id, message) = mock_sender.next_outbound().expect("No message was sent");
        assert_network_message(
            message,
            id.into(),
            "abc_network",
            CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
            |msg: CircuitDirectMessage| {
                assert_eq!(msg.get_sender(), "def");
                assert_eq!(msg.get_circuit(), "alpha");
                assert_eq!(msg.get_recipient(), "abc");
                assert_eq!(msg.get_payload().to_vec(), b"test".to_vec());
                assert_eq!(msg.get_correlation_id(), "1234");
            },
        )
    }

    // Test that a direct message will be properly sent to the node the recipient service is
    // connected to
    #[test]
    fn test_circuit_direct_message_handler_node() {
        // Set up dispatcher and mock sender
        let mock_sender = MockSender::new();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));

        // Add circuit and service to splinter state
        let circuit = Circuit::builder()
            .with_id("alpha".into())
            .with_auth(AuthorizationType::Trust)
            .with_members(vec!["123".into(), "345".into()])
            .with_roster(vec!["abc".into(), "def".into()])
            .with_persistence(PersistenceType::Any)
            .with_durability(DurabilityType::NoDurability)
            .with_routes(RouteType::Any)
            .with_circuit_management_type("circuit_direct_test_app".into())
            .build()
            .expect("Should have built a correct circuit");

        let mut circuit_directory = CircuitDirectory::new();
        circuit_directory.add_circuit("alpha".to_string(), circuit);

        let state = SplinterState::new("memory".to_string(), circuit_directory);

        let node_123 = SplinterNode::new("123".to_string(), vec!["123.0.0.1:0".to_string()]);
        let node_345 = SplinterNode::new("345".to_string(), vec!["123.0.0.1:0".to_string()]);

        let service_abc =
            Service::new("abc".to_string(), Some("abc_network".to_string()), node_123);
        let service_def =
            Service::new("def".to_string(), Some("def_network".to_string()), node_345);
        let abc_id = ServiceId::new("alpha".into(), "abc".into());
        let def_id = ServiceId::new("alpha".into(), "def".into());
        state.add_service(abc_id, service_abc).unwrap();
        state.add_service(def_id, service_def).unwrap();

        // Add direct message handler to dispatcher
        let handler = CircuitDirectMessageHandler::new("345".to_string(), state);

        dispatcher.set_handler(Box::new(handler));

        // create dispatch message
        let mut direct_message = CircuitDirectMessage::new();
        direct_message.set_circuit("alpha".into());
        direct_message.set_sender("def".into());
        direct_message.set_recipient("abc".into());
        direct_message.set_payload(b"test".to_vec());
        direct_message.set_correlation_id("1234".into());
        let direct_bytes = direct_message.write_to_bytes().unwrap();

        // dispatch the message
        dispatcher
            .dispatch(
                "def".into(),
                &CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
                direct_bytes.clone(),
            )
            .unwrap();

        let (id, message) = mock_sender.next_outbound().expect("No message was sent");
        assert_network_message(
            message,
            id.into(),
            "123",
            CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
            |msg: CircuitDirectMessage| {
                assert_eq!(msg.get_sender(), "def");
                assert_eq!(msg.get_circuit(), "alpha");
                assert_eq!(msg.get_recipient(), "abc");
                assert_eq!(msg.get_payload().to_vec(), b"test".to_vec());
                assert_eq!(msg.get_correlation_id(), "1234");
            },
        )
    }

    // Test that an error message is returned if the sender is not connected to the circuit
    #[test]
    fn test_circuit_direct_message_handler_sender_not_in_directory() {
        // Set up dispatcher and mock sender
        let mock_sender = MockSender::new();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));

        // add the circuit and service to splinter state
        let circuit = Circuit::builder()
            .with_id("alpha".into())
            .with_auth(AuthorizationType::Trust)
            .with_members(vec!["123".into()])
            .with_roster(vec!["abc".into(), "def".into()])
            .with_persistence(PersistenceType::Any)
            .with_durability(DurabilityType::NoDurability)
            .with_routes(RouteType::Any)
            .with_circuit_management_type("circuit_direct_test_app".into())
            .build()
            .expect("Should have built a correct circuit");

        let mut circuit_directory = CircuitDirectory::new();
        circuit_directory.add_circuit("alpha".to_string(), circuit);

        let state = SplinterState::new("memory".to_string(), circuit_directory);

        let node = SplinterNode::new("123".to_string(), vec!["123.0.0.1:0".to_string()]);
        let service_abc = Service::new(
            "abc".to_string(),
            Some("abc_network".to_string()),
            node.clone(),
        );
        let id = ServiceId::new("alpha".into(), "abc".into());
        state.add_service(id.clone(), service_abc).unwrap();

        // add direct message handler to the dispatcher
        let handler = CircuitDirectMessageHandler::new("123".to_string(), state);

        dispatcher.set_handler(Box::new(handler));

        // create direct message
        let mut direct_message = CircuitDirectMessage::new();
        direct_message.set_circuit("alpha".into());
        direct_message.set_sender("def".into());
        direct_message.set_recipient("abc".into());
        direct_message.set_payload(b"test".to_vec());
        direct_message.set_correlation_id("1234".into());
        let direct_bytes = direct_message.write_to_bytes().unwrap();

        // dispatcher message
        dispatcher
            .dispatch(
                "def".into(),
                &CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
                direct_bytes.clone(),
            )
            .unwrap();

        let (id, message) = mock_sender.next_outbound().expect("No message was sent");
        assert_network_message(
            message,
            id.into(),
            "def",
            CircuitMessageType::CIRCUIT_ERROR_MESSAGE,
            |msg: CircuitError| {
                assert_eq!(msg.get_service_id(), "def");
                assert_eq!(
                    msg.get_error(),
                    CircuitError_Error::ERROR_SENDER_NOT_IN_DIRECTORY
                );
                assert_eq!(msg.get_correlation_id(), "1234");
            },
        )
    }

    // Test that an error message is returned if the sender is not in the circuit roster
    #[test]
    fn test_circuit_direct_message_handler_sender_not_in_circuit_roster() {
        // Set up dispatcher and mock sender
        let mock_sender = MockSender::new();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));

        let circuit = Circuit::builder()
            .with_id("alpha".into())
            .with_auth(AuthorizationType::Trust)
            .with_members(vec!["123".into()])
            .with_roster(vec!["abc".into()])
            .with_persistence(PersistenceType::Any)
            .with_durability(DurabilityType::NoDurability)
            .with_routes(RouteType::Any)
            .with_circuit_management_type("circuit_direct_test_app".into())
            .build()
            .expect("Should have built a correct circuit");

        let mut circuit_directory = CircuitDirectory::new();
        circuit_directory.add_circuit("alpha".to_string(), circuit);

        let state = SplinterState::new("memory".to_string(), circuit_directory);

        let node = SplinterNode::new("123".to_string(), vec!["123.0.0.1:0".to_string()]);
        let service_abc = Service::new(
            "abc".to_string(),
            Some("abc_network".to_string()),
            node.clone(),
        );
        let id = ServiceId::new("alpha".into(), "abc".into());
        state.add_service(id.clone(), service_abc).unwrap();

        // add direct message handler to the dispatcher
        let handler = CircuitDirectMessageHandler::new("123".to_string(), state);

        dispatcher.set_handler(Box::new(handler));

        // create direct message
        let mut direct_message = CircuitDirectMessage::new();
        direct_message.set_circuit("alpha".into());
        direct_message.set_sender("def".into());
        direct_message.set_recipient("abc".into());
        direct_message.set_payload(b"test".to_vec());
        direct_message.set_correlation_id("1234".into());
        let direct_bytes = direct_message.write_to_bytes().unwrap();

        // dispatcher message
        dispatcher
            .dispatch(
                "def".into(),
                &CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
                direct_bytes.clone(),
            )
            .unwrap();

        let (id, message) = mock_sender.next_outbound().expect("No message was sent");
        assert_network_message(
            message,
            id.into(),
            "def",
            CircuitMessageType::CIRCUIT_ERROR_MESSAGE,
            |msg: CircuitError| {
                assert_eq!(msg.get_service_id(), "def");
                assert_eq!(
                    msg.get_error(),
                    CircuitError_Error::ERROR_SENDER_NOT_IN_CIRCUIT_ROSTER
                );
                assert_eq!(msg.get_correlation_id(), "1234");
            },
        )
    }

    // Test that if the recipient is not connected a circuit error message is sent back
    #[test]
    fn test_circuit_direct_message_handler_recipient_not_in_directory() {
        // Set up dispatcher and mock sender
        let mock_sender = MockSender::new();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));

        // add circuits and service to splinter state
        let circuit = Circuit::builder()
            .with_id("alpha".into())
            .with_auth(AuthorizationType::Trust)
            .with_members(vec!["123".into(), "345".into()])
            .with_roster(vec!["abc".into(), "def".into()])
            .with_persistence(PersistenceType::Any)
            .with_durability(DurabilityType::NoDurability)
            .with_routes(RouteType::Any)
            .with_circuit_management_type("circuit_direct_test_app".into())
            .build()
            .expect("Should have built a correct circuit");

        let mut circuit_directory = CircuitDirectory::new();
        circuit_directory.add_circuit("alpha".to_string(), circuit);

        let state = SplinterState::new("memory".to_string(), circuit_directory);

        let node_345 = SplinterNode::new("345".to_string(), vec!["123.0.0.1:0".to_string()]);
        let service_def =
            Service::new("def".to_string(), Some("def_network".to_string()), node_345);
        let id = ServiceId::new("alpha".into(), "def".into());
        state.add_service(id.clone(), service_def).unwrap();

        // add handler to dispatcher
        let handler = CircuitDirectMessageHandler::new("345".to_string(), state);
        dispatcher.set_handler(Box::new(handler));

        // create direct message
        let mut direct_message = CircuitDirectMessage::new();
        direct_message.set_circuit("alpha".into());
        direct_message.set_sender("def".into());
        direct_message.set_recipient("abc".into());
        direct_message.set_payload(b"test".to_vec());
        direct_message.set_correlation_id("1234".into());
        let direct_bytes = direct_message.write_to_bytes().unwrap();

        // dispatch message
        dispatcher
            .dispatch(
                "def".into(),
                &CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
                direct_bytes.clone(),
            )
            .unwrap();

        let (id, message) = mock_sender.next_outbound().expect("No message was sent");
        assert_network_message(
            message,
            id.into(),
            "def",
            CircuitMessageType::CIRCUIT_ERROR_MESSAGE,
            |msg: CircuitError| {
                assert_eq!(msg.get_service_id(), "def");
                assert_eq!(
                    msg.get_error(),
                    CircuitError_Error::ERROR_RECIPIENT_NOT_IN_DIRECTORY
                );
                assert_eq!(msg.get_correlation_id(), "1234");
            },
        )
    }

    // Test that an error message is returned if the recipient is not in the circuit roster
    #[test]
    fn test_circuit_direct_message_handler_recipient_not_in_circuit_roster() {
        // Set up dispatcher and mock sender
        let mock_sender = MockSender::new();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));

        // add circuits and service to splinter state
        let circuit = Circuit::builder()
            .with_id("alpha".into())
            .with_auth(AuthorizationType::Trust)
            .with_members(vec!["123".into(), "345".into()])
            .with_roster(vec!["def".into()])
            .with_persistence(PersistenceType::Any)
            .with_durability(DurabilityType::NoDurability)
            .with_routes(RouteType::Any)
            .with_circuit_management_type("circuit_direct_test_app".into())
            .build()
            .expect("Should have built a correct circuit");

        let mut circuit_directory = CircuitDirectory::new();
        circuit_directory.add_circuit("alpha".to_string(), circuit);

        let state = SplinterState::new("memory".to_string(), circuit_directory);

        let node_345 = SplinterNode::new("123".to_string(), vec!["123.0.0.1:0".to_string()]);
        let service_def =
            Service::new("def".to_string(), Some("def_network".to_string()), node_345);
        let id = ServiceId::new("alpha".into(), "def".into());
        state.add_service(id.clone(), service_def).unwrap();

        // add direct message handler
        let handler = CircuitDirectMessageHandler::new("345".to_string(), state);
        dispatcher.set_handler(Box::new(handler));

        // create direct message
        let mut direct_message = CircuitDirectMessage::new();
        direct_message.set_circuit("alpha".into());
        direct_message.set_sender("def".into());
        direct_message.set_recipient("abc".into());
        direct_message.set_payload(b"test".to_vec());
        direct_message.set_correlation_id("1234".into());
        let direct_bytes = direct_message.write_to_bytes().unwrap();

        // dispatch message
        dispatcher
            .dispatch(
                "def".into(),
                &CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
                direct_bytes.clone(),
            )
            .unwrap();

        let (id, message) = mock_sender.next_outbound().expect("No message was sent");
        assert_network_message(
            message,
            id.into(),
            "def",
            CircuitMessageType::CIRCUIT_ERROR_MESSAGE,
            |msg: CircuitError| {
                assert_eq!(msg.get_service_id(), "def");
                assert_eq!(
                    msg.get_error(),
                    CircuitError_Error::ERROR_RECIPIENT_NOT_IN_CIRCUIT_ROSTER
                );
                assert_eq!(msg.get_correlation_id(), "1234");
            },
        )
    }

    // Test that an error message is returned if the circuit does not exist
    #[test]
    fn test_circuit_direct_message_handler_no_circuit() {
        // Set up dispatcher and mock sender
        let mock_sender = MockSender::new();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));

        // create empty splinter state
        let circuit_directory = CircuitDirectory::new();

        let state = SplinterState::new("memory".to_string(), circuit_directory);

        // add direct message handler to the dispatcher
        let handler = CircuitDirectMessageHandler::new("345".to_string(), state);
        dispatcher.set_handler(Box::new(handler));

        // create direct message
        let mut direct_message = CircuitDirectMessage::new();
        direct_message.set_circuit("alpha".into());
        direct_message.set_sender("def".into());
        direct_message.set_recipient("abc".into());
        direct_message.set_payload(b"test".to_vec());
        direct_message.set_correlation_id("1234".into());
        let direct_bytes = direct_message.write_to_bytes().unwrap();

        // dispatch message
        dispatcher
            .dispatch(
                "def".into(),
                &CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
                direct_bytes.clone(),
            )
            .unwrap();

        let (id, message) = mock_sender.next_outbound().expect("No message was sent");
        assert_network_message(
            message,
            id.into(),
            "def",
            CircuitMessageType::CIRCUIT_ERROR_MESSAGE,
            |msg: CircuitError| {
                assert_eq!(msg.get_service_id(), "def");
                assert_eq!(
                    msg.get_error(),
                    CircuitError_Error::ERROR_CIRCUIT_DOES_NOT_EXIST
                );
                assert_eq!(msg.get_correlation_id(), "1234");
            },
        )
    }

    fn assert_network_message<M: protobuf::Message, F: Fn(M)>(
        message: Vec<u8>,
        recipient: String,
        expected_recipient: &str,
        expected_circuit_msg_type: CircuitMessageType,
        detail_assertions: F,
    ) {
        assert_eq!(expected_recipient, &recipient);

        let network_msg: NetworkMessage = protobuf::parse_from_bytes(&message).unwrap();
        let circuit_msg: CircuitMessage =
            protobuf::parse_from_bytes(network_msg.get_payload()).unwrap();
        assert_eq!(expected_circuit_msg_type, circuit_msg.get_message_type(),);
        let circuit_msg: M = protobuf::parse_from_bytes(circuit_msg.get_payload()).unwrap();

        detail_assertions(circuit_msg);
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
