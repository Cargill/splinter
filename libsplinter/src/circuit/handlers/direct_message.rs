// Copyright 2018-2022 Cargill Incorporated
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
use crate::circuit::routing::{RoutingTableReader, ServiceId as RoutingServiceId};
use crate::network::dispatch::{DispatchError, Handler, MessageContext, MessageSender, PeerId};
use crate::peer::PeerTokenPair;
use crate::protos::circuit::{
    CircuitDirectMessage, CircuitError, CircuitError_Error, CircuitMessageType,
};

#[cfg(feature = "service-message-handler-dispatch")]
use crate::error::InternalError;
#[cfg(feature = "service-message-handler-dispatch")]
use crate::{
    runtime::service::ServiceDispatcher,
    service::{CircuitId, FullyQualifiedServiceId, ServiceId},
};

use protobuf::Message;

// Implements a handler that handles CircuitDirectMessage
pub struct CircuitDirectMessageHandler {
    node_id: String,
    routing_table: Box<dyn RoutingTableReader>,
    #[cfg(feature = "service-message-handler-dispatch")]
    service_dispatcher: ServiceDispatcher,
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
        let recipient_id = RoutingServiceId::new(circuit_name.to_string(), recipient.to_string());

        #[cfg(feature = "service-message-handler-dispatch")]
        {
            let to_service = FullyQualifiedServiceId::new(
                CircuitId::new(circuit_name)
                    .map_err(|e| InternalError::from_source(Box::new(e)))?,
                ServiceId::new(msg.get_recipient())
                    .map_err(|e| InternalError::from_source(Box::new(e)))?,
            );

            if self.service_dispatcher.is_routable(&to_service)? {
                let from_service = FullyQualifiedServiceId::new(
                    CircuitId::new(circuit_name)
                        .map_err(|e| InternalError::from_source(Box::new(e)))?,
                    ServiceId::new(msg.get_sender())
                        .map_err(|e| InternalError::from_source(Box::new(e)))?,
                );

                let mut msg = msg;
                self.service_dispatcher
                    .dispatch(to_service, from_service, msg.take_payload())?;
                return Ok(());
            }
        }

        // msg bytes will either be message bytes of a direct message or an error message
        // the msg_recipient is either the service/node id to send the message to or is the
        // peer_id to send back the error message
        let (msg_bytes, msg_recipient) = {
            if let Some(circuit) = self
                .routing_table
                .get_circuit(circuit_name)
                .map_err(|err| DispatchError::HandleError(err.to_string()))?
            {
                // Check if the message sender is allowed on the circuit
                // if the sender is not allowed on the circuit
                if !circuit
                    .roster()
                    .iter()
                    .any(|service| service.service_id() == msg_sender)
                {
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
                    (network_msg_bytes, context.source_peer_id().clone())
                } else if circuit
                    .roster()
                    .iter()
                    .any(|service| service.service_id() == recipient)
                {
                    // check if the recipient service is allowed on the circuit and registered
                    if let Some(service) = self
                        .routing_table
                        .get_service(&recipient_id)
                        .map_err(|err| DispatchError::HandleError(err.to_string()))?
                    {
                        let node_id = service.node_id().to_string();
                        let msg_bytes = context.message_bytes().to_vec();
                        let network_msg_bytes =
                            create_message(msg_bytes, CircuitMessageType::CIRCUIT_DIRECT_MESSAGE)?;
                        // If the service is on this node send message to the service, otherwise
                        // send the message to the node the service is connected to
                        if node_id != self.node_id {
                            let node_peer_id: PeerId = {
                                let peer_id = self
                                    .routing_table
                                    .get_node(&node_id)
                                    .map_err(|err| DispatchError::HandleError(err.to_string()))?
                                    .ok_or_else(|| {
                                        DispatchError::HandleError(format!(
                                            "Node {} not in routing table",
                                            node_id
                                        ))
                                    })?
                                    .get_peer_auth_token(circuit.authorization_type())
                                    .map_err(|err| DispatchError::HandleError(err.to_string()))?;

                                let local_peer_id = self
                                    .routing_table
                                    .get_node(&self.node_id)
                                    .map_err(|err| DispatchError::HandleError(err.to_string()))?
                                    .ok_or_else(|| {
                                        DispatchError::HandleError(format!(
                                            "Local Node {} not in routing table",
                                            node_id
                                        ))
                                    })?
                                    .get_peer_auth_token(circuit.authorization_type())
                                    .map_err(|err| DispatchError::HandleError(err.to_string()))?;

                                PeerTokenPair::new(peer_id, local_peer_id)
                            }
                            .into();

                            (network_msg_bytes, node_peer_id)
                        } else {
                            let peer_id: PeerId = match service.local_peer_id() {
                                Some(peer_id) => peer_id.clone().into(),
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
                        (network_msg_bytes, context.source_peer_id().clone())
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
                    (network_msg_bytes, context.source_peer_id().clone())
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
                (network_msg_bytes, context.source_peer_id().clone())
            }
        };

        // either forward the direct message or send back an error message.
        sender
            .send(msg_recipient, msg_bytes)
            .map_err(|(recipient, payload)| {
                DispatchError::NetworkSendError((recipient.into(), payload))
            })?;
        Ok(())
    }
}

impl CircuitDirectMessageHandler {
    pub fn new(
        node_id: String,
        routing_table: Box<dyn RoutingTableReader>,
        #[cfg(feature = "service-message-handler-dispatch")] service_dispatcher: ServiceDispatcher,
    ) -> Self {
        CircuitDirectMessageHandler {
            node_id,
            routing_table,
            #[cfg(feature = "service-message-handler-dispatch")]
            service_dispatcher,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use crate::circuit::routing::AuthorizationType;
    use crate::circuit::routing::{
        memory::RoutingTable, Circuit, CircuitNode, RoutingTableWriter, Service,
    };
    use crate::network::dispatch::Dispatcher;
    use crate::peer::PeerAuthorizationToken;
    use crate::protos::circuit::CircuitMessage;
    use crate::protos::network::NetworkMessage;

    #[cfg(feature = "service-message-handler-dispatch")]
    use crate::runtime::service::{
        NetworkMessageSenderFactory, RoutingTableServiceTypeResolver,
        SingleThreadedMessageHandlerTaskRunner,
    };
    #[cfg(feature = "service-message-handler-dispatch")]
    use crate::service::{
        MessageHandler, MessageHandlerFactory, MessageSender as ServiceMessageSender, Routable,
        ServiceType,
    };

    // Test that a direct message will be properly sent to the service if the message is meant for
    // a service connected to the receiving node
    #[test]
    fn test_circuit_direct_message_handler_service() {
        // Set up dispatcher and mock sender
        let mock_sender = MockSender::new();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));

        let table = RoutingTable::default();
        let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
        let mut writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let node_123 = CircuitNode::new("123".to_string(), vec!["123.0.0.1:0".to_string()], None);
        let node_345 = CircuitNode::new("345".to_string(), vec!["123.0.0.1:1".to_string()], None);

        let mut service_abc = Service::new(
            "b0001".to_string(),
            "test".to_string(),
            "123".to_string(),
            vec![],
        );
        let mut service_def = Service::new(
            "a0001".to_string(),
            "test".to_string(),
            "345".to_string(),
            vec![],
        );

        service_abc.set_local_peer_id(PeerTokenPair::new(
            PeerAuthorizationToken::from_peer_id("abc_network"),
            PeerAuthorizationToken::from_peer_id("123"),
        ));
        service_def.set_local_peer_id(PeerTokenPair::new(
            PeerAuthorizationToken::from_peer_id("def_network"),
            PeerAuthorizationToken::from_peer_id("345"),
        ));

        // Add circuit and service to splinter state
        let circuit = Circuit::new(
            "Alpha-00000".into(),
            vec![service_abc.clone(), service_def.clone()],
            vec!["123".into(), "345".into()],
            AuthorizationType::Trust,
        );

        writer
            .add_circuit(
                circuit.circuit_id().into(),
                circuit,
                vec![node_123, node_345],
            )
            .expect("Unable to add circuits");

        // Add direct message handler to the the dispatcher
        let handler = CircuitDirectMessageHandler::new(
            "123".to_string(),
            reader.clone(),
            #[cfg(feature = "service-message-handler-dispatch")]
            new_service_dispatcher(mock_sender.clone(), reader),
        );
        dispatcher.set_handler(Box::new(handler));

        // Create the direct message
        let mut direct_message = CircuitDirectMessage::new();
        direct_message.set_circuit("Alpha-00000".into());
        direct_message.set_sender("a0001".into());
        direct_message.set_recipient("b0001".into());
        direct_message.set_payload(b"test".to_vec());
        direct_message.set_correlation_id("1234".into());
        let direct_bytes = direct_message.write_to_bytes().unwrap();

        // dispatch the direct message
        dispatcher
            .dispatch(
                PeerTokenPair::new(
                    PeerAuthorizationToken::from_peer_id("def"),
                    PeerAuthorizationToken::from_peer_id("345"),
                )
                .into(),
                &CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
                direct_bytes.clone(),
            )
            .unwrap();

        let (id, message) = mock_sender.next_outbound().expect("No message was sent");
        assert_network_message(
            message,
            id.into(),
            PeerTokenPair::new(
                PeerAuthorizationToken::from_peer_id("abc_network"),
                PeerAuthorizationToken::from_peer_id("123"),
            ),
            CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
            |msg: CircuitDirectMessage| {
                assert_eq!(msg.get_sender(), "a0001");
                assert_eq!(msg.get_circuit(), "Alpha-00000");
                assert_eq!(msg.get_recipient(), "b0001");
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

        let table = RoutingTable::default();
        let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
        let mut writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let node_123 = CircuitNode::new("123".to_string(), vec!["123.0.0.1:0".to_string()], None);
        let node_345 = CircuitNode::new("345".to_string(), vec!["123.0.0.1:1".to_string()], None);

        let mut service_abc = Service::new(
            "b0001".to_string(),
            "test".to_string(),
            "123".to_string(),
            vec![],
        );
        let mut service_def = Service::new(
            "a0001".to_string(),
            "test".to_string(),
            "345".to_string(),
            vec![],
        );

        service_abc.set_local_peer_id(PeerTokenPair::new(
            PeerAuthorizationToken::from_peer_id("abc_network"),
            PeerAuthorizationToken::from_peer_id("123"),
        ));
        service_def.set_local_peer_id(PeerTokenPair::new(
            PeerAuthorizationToken::from_peer_id("def_network"),
            PeerAuthorizationToken::from_peer_id("345"),
        ));

        // Add circuit and service to splinter state
        let circuit = Circuit::new(
            "Alpha-00000".into(),
            vec![service_abc.clone(), service_def.clone()],
            vec!["123".into(), "345".into()],
            AuthorizationType::Trust,
        );

        writer
            .add_circuit(
                circuit.circuit_id().into(),
                circuit,
                vec![node_123, node_345],
            )
            .expect("Unable to add circuits");

        // Add direct message handler to dispatcher
        let handler = CircuitDirectMessageHandler::new(
            "345".to_string(),
            reader.clone(),
            #[cfg(feature = "service-message-handler-dispatch")]
            new_service_dispatcher(mock_sender.clone(), reader),
        );

        dispatcher.set_handler(Box::new(handler));

        // create dispatch message
        let mut direct_message = CircuitDirectMessage::new();
        direct_message.set_circuit("Alpha-00000".into());
        direct_message.set_sender("a0001".into());
        direct_message.set_recipient("b0001".into());
        direct_message.set_payload(b"test".to_vec());
        direct_message.set_correlation_id("1234".into());
        let direct_bytes = direct_message.write_to_bytes().unwrap();

        // dispatch the message
        dispatcher
            .dispatch(
                PeerTokenPair::new(
                    PeerAuthorizationToken::from_peer_id("def"),
                    PeerAuthorizationToken::from_peer_id("345"),
                )
                .into(),
                &CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
                direct_bytes.clone(),
            )
            .unwrap();

        let (id, message) = mock_sender.next_outbound().expect("No message was sent");
        assert_network_message(
            message,
            id.into(),
            PeerTokenPair::new(
                PeerAuthorizationToken::from_peer_id("123"),
                PeerAuthorizationToken::from_peer_id("345"),
            ),
            CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
            |msg: CircuitDirectMessage| {
                assert_eq!(msg.get_sender(), "a0001");
                assert_eq!(msg.get_circuit(), "Alpha-00000");
                assert_eq!(msg.get_recipient(), "b0001");
                assert_eq!(msg.get_payload().to_vec(), b"test".to_vec());
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

        let table = RoutingTable::default();
        let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
        let mut writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let node_123 = CircuitNode::new("123".to_string(), vec!["123.0.0.1:0".to_string()], None);
        let node_345 = CircuitNode::new("345".to_string(), vec!["123.0.0.1:1".to_string()], None);

        let mut service_abc = Service::new(
            "b0001".to_string(),
            "test".to_string(),
            "123".to_string(),
            vec![],
        );

        service_abc.set_local_peer_id(PeerTokenPair::new(
            PeerAuthorizationToken::from_peer_id("abc_network"),
            PeerAuthorizationToken::from_peer_id("123"),
        ));

        // Add circuit and service to splinter state
        let circuit = Circuit::new(
            "Alpha-00000".into(),
            vec![service_abc.clone()],
            vec!["123".into(), "345".into()],
            AuthorizationType::Trust,
        );

        writer
            .add_circuit(
                circuit.circuit_id().into(),
                circuit,
                vec![node_123, node_345],
            )
            .expect("Unable to add circuits");

        // add direct message handler to the dispatcher
        let handler = CircuitDirectMessageHandler::new(
            "123".to_string(),
            reader.clone(),
            #[cfg(feature = "service-message-handler-dispatch")]
            new_service_dispatcher(mock_sender.clone(), reader),
        );

        dispatcher.set_handler(Box::new(handler));

        // create direct message
        let mut direct_message = CircuitDirectMessage::new();
        direct_message.set_circuit("Alpha-00000".into());
        direct_message.set_sender("a0001".into());
        direct_message.set_recipient("b0001".into());
        direct_message.set_payload(b"test".to_vec());
        direct_message.set_correlation_id("1234".into());
        let direct_bytes = direct_message.write_to_bytes().unwrap();

        // dispatcher message
        dispatcher
            .dispatch(
                PeerTokenPair::new(
                    PeerAuthorizationToken::from_peer_id("def"),
                    PeerAuthorizationToken::from_peer_id("345"),
                )
                .into(),
                &CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
                direct_bytes.clone(),
            )
            .unwrap();

        let (id, message) = mock_sender.next_outbound().expect("No message was sent");
        assert_network_message(
            message,
            id.into(),
            PeerTokenPair::new(
                PeerAuthorizationToken::from_peer_id("def"),
                PeerAuthorizationToken::from_peer_id("345"),
            ),
            CircuitMessageType::CIRCUIT_ERROR_MESSAGE,
            |msg: CircuitError| {
                assert_eq!(msg.get_service_id(), "a0001");
                assert_eq!(
                    msg.get_error(),
                    CircuitError_Error::ERROR_SENDER_NOT_IN_CIRCUIT_ROSTER
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

        let table = RoutingTable::default();
        let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
        let mut writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let node_123 = CircuitNode::new("123".to_string(), vec!["123.0.0.1:0".to_string()], None);
        let node_345 = CircuitNode::new("345".to_string(), vec!["123.0.0.1:1".to_string()], None);

        let mut service_def = Service::new(
            "a0001".to_string(),
            "test".to_string(),
            "345".to_string(),
            vec![],
        );

        service_def.set_local_peer_id(PeerTokenPair::new(
            PeerAuthorizationToken::from_peer_id("def_network"),
            PeerAuthorizationToken::from_peer_id("345"),
        ));

        // Add circuit and service to splinter state
        let circuit = Circuit::new(
            "Alpha-00000".into(),
            vec![service_def.clone()],
            vec!["123".into(), "345".into()],
            AuthorizationType::Trust,
        );

        writer
            .add_circuit(
                circuit.circuit_id().into(),
                circuit,
                vec![node_123, node_345],
            )
            .expect("Unable to add circuits");

        // add direct message handler
        let handler = CircuitDirectMessageHandler::new(
            "345".to_string(),
            reader.clone(),
            #[cfg(feature = "service-message-handler-dispatch")]
            new_service_dispatcher(mock_sender.clone(), reader),
        );
        dispatcher.set_handler(Box::new(handler));

        // create direct message
        let mut direct_message = CircuitDirectMessage::new();
        direct_message.set_circuit("Alpha-00000".into());
        direct_message.set_sender("a0001".into());
        direct_message.set_recipient("b0001".into());
        direct_message.set_payload(b"test".to_vec());
        direct_message.set_correlation_id("1234".into());
        let direct_bytes = direct_message.write_to_bytes().unwrap();

        // dispatch message
        dispatcher
            .dispatch(
                PeerTokenPair::new(
                    PeerAuthorizationToken::from_peer_id("def"),
                    PeerAuthorizationToken::from_peer_id("345"),
                )
                .into(),
                &CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
                direct_bytes.clone(),
            )
            .unwrap();

        let (id, message) = mock_sender.next_outbound().expect("No message was sent");
        assert_network_message(
            message,
            id.into(),
            PeerTokenPair::new(
                PeerAuthorizationToken::from_peer_id("def"),
                PeerAuthorizationToken::from_peer_id("345"),
            ),
            CircuitMessageType::CIRCUIT_ERROR_MESSAGE,
            |msg: CircuitError| {
                assert_eq!(msg.get_service_id(), "a0001");
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

        let table = RoutingTable::default();
        let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());

        // add direct message handler to the dispatcher
        let handler = CircuitDirectMessageHandler::new(
            "345".to_string(),
            reader.clone(),
            #[cfg(feature = "service-message-handler-dispatch")]
            new_service_dispatcher(mock_sender.clone(), reader),
        );
        dispatcher.set_handler(Box::new(handler));

        // create direct message
        let mut direct_message = CircuitDirectMessage::new();
        direct_message.set_circuit("Alpha-00000".into());
        direct_message.set_sender("a0001".into());
        direct_message.set_recipient("b0001".into());
        direct_message.set_payload(b"test".to_vec());
        direct_message.set_correlation_id("1234".into());
        let direct_bytes = direct_message.write_to_bytes().unwrap();

        // dispatch message
        dispatcher
            .dispatch(
                PeerTokenPair::new(
                    PeerAuthorizationToken::from_peer_id("def"),
                    PeerAuthorizationToken::from_peer_id("345"),
                )
                .into(),
                &CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
                direct_bytes.clone(),
            )
            .unwrap();

        let (id, message) = mock_sender.next_outbound().expect("No message was sent");
        assert_network_message(
            message,
            id.into(),
            PeerTokenPair::new(
                PeerAuthorizationToken::from_peer_id("def"),
                PeerAuthorizationToken::from_peer_id("345"),
            ),
            CircuitMessageType::CIRCUIT_ERROR_MESSAGE,
            |msg: CircuitError| {
                assert_eq!(msg.get_service_id(), "a0001");
                assert_eq!(
                    msg.get_error(),
                    CircuitError_Error::ERROR_CIRCUIT_DOES_NOT_EXIST
                );
                assert_eq!(msg.get_correlation_id(), "1234");
            },
        )
    }

    #[cfg(feature = "service-message-handler-dispatch")]
    #[test]
    fn test_circuit_direct_message_handler_via_service_dispatcher() {
        // Set up dispatcher and mock sender
        let mock_sender = MockSender::new();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));

        let table = RoutingTable::default();
        let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
        let mut writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let node_123 = CircuitNode::new("123".to_string(), vec!["123.0.0.1:0".to_string()], None);
        let node_345 = CircuitNode::new("345".to_string(), vec!["123.0.0.1:1".to_string()], None);

        let service_abc = Service::new(
            "b0001".to_string(),
            "testservice2".to_string(),
            "123".to_string(),
            vec![],
        );
        let service_def = Service::new(
            "a0001".to_string(),
            "testservice2".to_string(),
            "345".to_string(),
            vec![],
        );

        // Add circuit and service to splinter state
        let circuit = Circuit::new(
            "Alpha-00000".into(),
            vec![service_abc.clone(), service_def.clone()],
            vec!["123".into(), "345".into()],
            AuthorizationType::Trust,
        );

        writer
            .add_circuit(
                circuit.circuit_id().into(),
                circuit,
                vec![node_123, node_345],
            )
            .expect("Unable to add circuits");

        // add direct message handler to the dispatcher
        let handler = CircuitDirectMessageHandler::new(
            "345".to_string(),
            reader.clone(),
            new_service_dispatcher_with_handler(mock_sender.clone(), reader, "testservice2"),
        );
        dispatcher.set_handler(Box::new(handler));

        // Create the direct message
        let mut direct_message = CircuitDirectMessage::new();
        direct_message.set_circuit("Alpha-00000".into());
        direct_message.set_sender("b0001".into());
        direct_message.set_recipient("a0001".into());
        direct_message.set_payload(b"test".to_vec());
        let direct_bytes = direct_message.write_to_bytes().unwrap();

        // dispatch the direct message
        dispatcher
            .dispatch(
                PeerTokenPair::new(
                    PeerAuthorizationToken::from_peer_id("123"),
                    PeerAuthorizationToken::from_peer_id("345"),
                )
                .into(),
                &CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
                direct_bytes.clone(),
            )
            .unwrap();

        let (id, message) = mock_sender.next_outbound().expect("No message was sent");
        assert_network_message(
            message,
            id.into(),
            PeerTokenPair::new(
                PeerAuthorizationToken::from_peer_id("123"),
                PeerAuthorizationToken::from_peer_id("345"),
            ),
            CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
            |msg: CircuitDirectMessage| {
                // test that the correct echo message has been sent
                assert_eq!(msg.get_sender(), "a0001");
                assert_eq!(msg.get_circuit(), "Alpha-00000");
                assert_eq!(msg.get_recipient(), "b0001");
                assert_eq!(msg.get_payload().to_vec(), b"test;out".to_vec());
            },
        )
    }

    fn assert_network_message<M: protobuf::Message, F: Fn(M)>(
        message: Vec<u8>,
        recipient: PeerTokenPair,
        expected_recipient: PeerTokenPair,
        expected_circuit_msg_type: CircuitMessageType,
        detail_assertions: F,
    ) {
        assert_eq!(expected_recipient, recipient);

        let network_msg: NetworkMessage = Message::parse_from_bytes(&message).unwrap();
        let circuit_msg: CircuitMessage =
            Message::parse_from_bytes(network_msg.get_payload()).unwrap();
        assert_eq!(expected_circuit_msg_type, circuit_msg.get_message_type(),);
        let circuit_msg: M = Message::parse_from_bytes(circuit_msg.get_payload()).unwrap();

        detail_assertions(circuit_msg);
    }

    #[cfg(feature = "service-message-handler-dispatch")]
    fn new_service_dispatcher(
        sender: MockSender,
        routing_table_reader: Box<dyn RoutingTableReader>,
    ) -> ServiceDispatcher {
        ServiceDispatcher::new(
            vec![],
            Box::new(NetworkMessageSenderFactory::new(
                "345",
                sender,
                routing_table_reader.clone(),
            )),
            Box::new(RoutingTableServiceTypeResolver::new(routing_table_reader)),
            Box::new(SingleThreadedMessageHandlerTaskRunner::new()),
        )
    }

    #[cfg(feature = "service-message-handler-dispatch")]
    fn new_service_dispatcher_with_handler(
        sender: MockSender,
        routing_table_reader: Box<dyn RoutingTableReader>,
        service_type: &'static str,
    ) -> ServiceDispatcher {
        ServiceDispatcher::new(
            vec![
                TestMessageHandlerFactory::new(ServiceType::new_static(service_type)).into_boxed(),
            ],
            Box::new(NetworkMessageSenderFactory::new(
                "345",
                sender,
                routing_table_reader.clone(),
            )),
            Box::new(RoutingTableServiceTypeResolver::new(routing_table_reader)),
            Box::new(SingleThreadedMessageHandlerTaskRunner::new()),
        )
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

    #[cfg(feature = "service-message-handler-dispatch")]
    #[derive(Clone)]
    struct TestMessageHandlerFactory {
        service_types: Vec<ServiceType<'static>>,
    }

    #[cfg(feature = "service-message-handler-dispatch")]
    impl TestMessageHandlerFactory {
        fn new(service_type: ServiceType<'static>) -> Self {
            Self {
                service_types: vec![service_type],
            }
        }
    }

    #[cfg(feature = "service-message-handler-dispatch")]
    impl MessageHandlerFactory for TestMessageHandlerFactory {
        type MessageHandler = TestMessageHandler;

        fn new_handler(&self) -> Self::MessageHandler {
            TestMessageHandler
        }

        fn clone_boxed(
            &self,
        ) -> Box<dyn MessageHandlerFactory<MessageHandler = Self::MessageHandler>> {
            Box::new(self.clone())
        }
    }

    #[cfg(feature = "service-message-handler-dispatch")]
    impl Routable for TestMessageHandlerFactory {
        fn service_types(&self) -> &[ServiceType] {
            &self.service_types
        }
    }

    #[cfg(feature = "service-message-handler-dispatch")]
    struct TestMessageHandler;

    #[cfg(feature = "service-message-handler-dispatch")]
    impl MessageHandler for TestMessageHandler {
        type Message = Vec<u8>;

        fn handle_message(
            &mut self,
            sender: &dyn ServiceMessageSender<Self::Message>,
            _to_service: FullyQualifiedServiceId,
            from_service: FullyQualifiedServiceId,
            message: Self::Message,
        ) -> Result<(), InternalError> {
            let mut msg = message;
            msg.extend(b";out");

            sender.send(from_service.service_id(), msg)
        }
    }
}
