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

use crate::circuit::handlers::create_message;
use crate::circuit::routing::{RoutingTableReader, RoutingTableWriter, Service, ServiceId};
use crate::network::dispatch::{DispatchError, Handler, MessageContext, MessageSender, PeerId};
use crate::protos::circuit::{
    CircuitMessageType, ServiceConnectRequest, ServiceConnectResponse,
    ServiceConnectResponse_Status, ServiceDisconnectRequest, ServiceDisconnectResponse,
    ServiceDisconnectResponse_Status,
};

use protobuf::Message;

const ADMIN_SERVICE_ID_PREFIX: &str = "admin::";
const ADMIN_CIRCUIT_ID: &str = "admin";

// Implements a handler that handles ServiceConnectRequest
pub struct ServiceConnectRequestHandler {
    node_id: String,
    routing_table_reader: Box<dyn RoutingTableReader>,
    routing_table_writer: Box<dyn RoutingTableWriter>,
}

impl Handler for ServiceConnectRequestHandler {
    type Source = PeerId;
    type MessageType = CircuitMessageType;
    type Message = ServiceConnectRequest;

    fn match_type(&self) -> Self::MessageType {
        CircuitMessageType::SERVICE_CONNECT_REQUEST
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        debug!("Handle Service Connect Request {:?}", msg);
        let circuit_name = msg.get_circuit();
        let service_id = msg.get_service_id();
        let unique_id = ServiceId::new(circuit_name.to_string(), service_id.to_string());

        let mut response = ServiceConnectResponse::new();
        response.set_correlation_id(msg.get_correlation_id().into());
        response.set_circuit(circuit_name.into());
        response.set_service_id(service_id.into());

        // hold on to the write lock for the entirety of the function
        let circuit_result = self
            .routing_table_reader
            .get_circuit(circuit_name)
            .map_err(|err| DispatchError::HandleError(err.to_string()))?;

        if let Some(circuit) = circuit_result {
            if circuit
                .roster()
                .iter()
                .any(|service| service.service_id() == service_id)
                || service_id.starts_with(ADMIN_SERVICE_ID_PREFIX)
            {
                let mut service = {
                    if !service_id.starts_with(ADMIN_SERVICE_ID_PREFIX) {
                        self.routing_table_reader
                            .get_service(&unique_id)
                            .map_err(|err| DispatchError::HandleError(err.to_string()))?
                            .ok_or_else(|| {
                                DispatchError::HandleError(format!(
                                    "Unable to get service {}",
                                    service_id
                                ))
                            })?
                    } else {
                        Service::new(
                            service_id.to_string(),
                            ADMIN_CIRCUIT_ID.to_string(),
                            self.node_id.to_string(),
                            vec![],
                        )
                    }
                };

                // If the circuit exists and has the service in the roster but the service is already
                // connected, return an error response
                if service.peer_id().is_some() {
                    response.set_status(
                        ServiceConnectResponse_Status::ERROR_SERVICE_ALREADY_REGISTERED,
                    );
                    response
                        .set_error_message(format!("Service is already registered: {}", service_id))
                } else if service.node_id() != self.node_id {
                    response.set_status(ServiceConnectResponse_Status::ERROR_NOT_AN_ALLOWED_NODE);
                    response.set_error_message(format!("{} is not allowed on this node", unique_id))
                } else {
                    service.set_peer_id(context.source_peer_id().to_string());
                    let mut writer = self.routing_table_writer.clone();
                    writer
                        .add_service(unique_id, service)
                        .map_err(|err| DispatchError::HandleError(err.to_string()))?;

                    response.set_status(ServiceConnectResponse_Status::OK);
                }
            // If the circuit exists but does not have the service in its roster, return an error
            // response
            } else {
                response.set_status(
                    ServiceConnectResponse_Status::ERROR_SERVICE_NOT_IN_CIRCUIT_REGISTRY,
                );
                response.set_error_message(format!(
                    "Service is not allowed in the circuit: {}:{}",
                    circuit_name, service_id
                ))
            }
        // If the circuit does not exists, return an error response
        } else {
            response.set_status(ServiceConnectResponse_Status::ERROR_CIRCUIT_DOES_NOT_EXIST);
            response.set_error_message(format!("Circuit does not exist: {}", msg.get_circuit()))
        }

        // Return response
        let response_bytes = response.write_to_bytes()?;
        let network_msg_bytes =
            create_message(response_bytes, CircuitMessageType::SERVICE_CONNECT_RESPONSE)?;

        let recipient = context.source_peer_id().to_string();

        sender
            .send(recipient.into(), network_msg_bytes)
            .map_err(|(recipient, payload)| {
                DispatchError::NetworkSendError((recipient.into(), payload))
            })?;
        Ok(())
    }
}

impl ServiceConnectRequestHandler {
    pub fn new(
        node_id: String,
        routing_table_reader: Box<dyn RoutingTableReader>,
        routing_table_writer: Box<dyn RoutingTableWriter>,
    ) -> Self {
        ServiceConnectRequestHandler {
            node_id,
            routing_table_reader,
            routing_table_writer,
        }
    }
}

// Implements a handler that handles ServiceDisconnectRequest
pub struct ServiceDisconnectRequestHandler {
    routing_table_reader: Box<dyn RoutingTableReader>,
    routing_table_writer: Box<dyn RoutingTableWriter>,
}

impl Handler for ServiceDisconnectRequestHandler {
    type Source = PeerId;
    type MessageType = CircuitMessageType;
    type Message = ServiceDisconnectRequest;

    fn match_type(&self) -> Self::MessageType {
        CircuitMessageType::SERVICE_DISCONNECT_REQUEST
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        debug!("Handle Service Disconnect Request {:?}", msg);
        let circuit_name = msg.get_circuit();
        let service_id = msg.get_service_id();
        let unique_id = ServiceId::new(circuit_name.to_string(), service_id.to_string());

        let mut response = ServiceDisconnectResponse::new();
        response.set_correlation_id(msg.get_correlation_id().into());
        response.set_circuit(circuit_name.into());
        response.set_service_id(service_id.into());

        let circuit_result = self
            .routing_table_reader
            .get_circuit(circuit_name)
            .map_err(|err| DispatchError::HandleError(err.to_string()))?;

        if let Some(circuit) = circuit_result {
            // If the circuit has the service in its roster and the service is connected
            // forward the disconnection to the rest of the nodes on the circuit and remove the
            // service from splinter state
            if circuit
                .roster()
                .iter()
                .any(|service| service.service_id() == service_id)
                || service_id.starts_with(ADMIN_SERVICE_ID_PREFIX)
            {
                let mut service = self
                    .routing_table_reader
                    .get_service(&unique_id)
                    .map_err(|err| DispatchError::HandleError(err.to_string()))?
                    .ok_or_else(|| {
                        DispatchError::HandleError(format!("Unable to get service {}", service_id))
                    })?;

                if service.peer_id().is_some() {
                    service.remove_peer_id();
                    let mut writer = self.routing_table_writer.clone();
                    writer
                        .add_service(unique_id, service)
                        .map_err(|err| DispatchError::HandleError(err.to_string()))?;
                    response.set_status(ServiceDisconnectResponse_Status::OK);
                } else {
                    response
                        .set_status(ServiceDisconnectResponse_Status::ERROR_SERVICE_NOT_REGISTERED);
                    response.set_error_message(format!("Service is not registered: {}", service_id))
                }
            // If the circuit exists but does not have the service in its roster, return an error
            // response
            } else {
                response.set_status(
                    ServiceDisconnectResponse_Status::ERROR_SERVICE_NOT_IN_CIRCUIT_REGISTRY,
                );
                response.set_error_message(format!(
                    "Service is not allowed in the circuit: {}:{}",
                    circuit_name, service_id
                ))
            }
        // If the circuit does not exists, return an error response
        } else {
            response.set_status(ServiceDisconnectResponse_Status::ERROR_CIRCUIT_DOES_NOT_EXIST);
            response.set_error_message(format!("Circuit does not exist: {}", msg.get_circuit()))
        }

        // Return response
        let response_bytes = response.write_to_bytes()?;
        let network_msg_bytes = create_message(
            response_bytes,
            CircuitMessageType::SERVICE_DISCONNECT_RESPONSE,
        )?;

        let recipient = context.source_peer_id().to_string();
        sender
            .send(recipient.into(), network_msg_bytes)
            .map_err(|(recipient, payload)| {
                DispatchError::NetworkSendError((recipient.into(), payload))
            })?;
        Ok(())
    }
}

impl ServiceDisconnectRequestHandler {
    pub fn new(
        routing_table_reader: Box<dyn RoutingTableReader>,
        routing_table_writer: Box<dyn RoutingTableWriter>,
    ) -> Self {
        ServiceDisconnectRequestHandler {
            routing_table_reader,
            routing_table_writer,
        }
    }
}

impl From<protobuf::error::ProtobufError> for DispatchError {
    fn from(e: protobuf::error::ProtobufError) -> Self {
        DispatchError::SerializationError(e.to_string())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use crate::circuit::routing::{
        memory::RoutingTable, Circuit, CircuitNode, RoutingTableWriter, Service,
    };
    use crate::network::dispatch::Dispatcher;
    use crate::protos::circuit::CircuitMessage;
    use crate::protos::network::NetworkMessage;

    #[test]
    // Test that if the circuit does not exist, a ServiceConnectResponse is returned with
    // a ERROR_CIRCUIT_DOES_NOT_EXIST
    fn test_service_connect_request_handler_no_circuit() {
        // Set up dispatcher and mock sender
        let mock_sender = MockSender::new();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));

        let table = RoutingTable::default();
        let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let handler = ServiceConnectRequestHandler::new("123".to_string(), reader, writer);

        dispatcher.set_handler(Box::new(handler));
        let mut connect_request = ServiceConnectRequest::new();
        connect_request.set_circuit("alpha".into());
        connect_request.set_service_id("abc".into());
        let connect_bytes = connect_request.write_to_bytes().unwrap();

        dispatcher
            .dispatch(
                "abc".into(),
                &CircuitMessageType::SERVICE_CONNECT_REQUEST,
                connect_bytes.clone(),
            )
            .unwrap();

        let (id, message) = mock_sender.next_outbound().expect("No message was sent");
        assert_network_message(
            message,
            id.into(),
            "abc",
            CircuitMessageType::SERVICE_CONNECT_RESPONSE,
            |msg: ServiceConnectResponse| {
                assert_eq!(msg.get_service_id(), "abc");
                assert_eq!(msg.get_circuit(), "alpha");
                assert_eq!(
                    msg.get_status(),
                    ServiceConnectResponse_Status::ERROR_CIRCUIT_DOES_NOT_EXIST
                );
            },
        )
    }

    #[test]
    // Test that if the service is not in circuit, a ServiceConnectResponse is returned with
    // a ERROR_SERVICE_NOT_IN_CIRCUIT_REGISTRY
    fn test_service_connect_request_handler_not_in_circuit() {
        // Set up dispatcher and mock sender
        let mock_sender = MockSender::new();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));

        let (circuit, nodes) = build_circuit();

        let table = RoutingTable::default();
        let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
        let mut writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        writer
            .add_circuit(circuit.circuit_id().to_string(), circuit, nodes)
            .expect("Unable to add circuit");
        let handler = ServiceConnectRequestHandler::new("123".to_string(), reader, writer);

        dispatcher.set_handler(Box::new(handler));
        let mut connect_request = ServiceConnectRequest::new();
        connect_request.set_circuit("alpha".into());
        connect_request.set_service_id("BAD".into());
        let connect_bytes = connect_request.write_to_bytes().unwrap();

        dispatcher
            .dispatch(
                "BAD".into(),
                &CircuitMessageType::SERVICE_CONNECT_REQUEST,
                connect_bytes.clone(),
            )
            .unwrap();

        let (id, message) = mock_sender.next_outbound().expect("No message was sent");
        assert_network_message(
            message,
            id.into(),
            "BAD",
            CircuitMessageType::SERVICE_CONNECT_RESPONSE,
            |msg: ServiceConnectResponse| {
                assert_eq!(msg.get_service_id(), "BAD");
                assert_eq!(msg.get_circuit(), "alpha");
                assert_eq!(
                    msg.get_status(),
                    ServiceConnectResponse_Status::ERROR_SERVICE_NOT_IN_CIRCUIT_REGISTRY
                );
            },
        )
    }

    #[test]
    // Test that if the service is in a circuit and not connected, a ServiceConnectResponse is
    // returned with an OK
    fn test_service_connect_request_handler() {
        // Set up dispatcher and mock sender
        let mock_sender = MockSender::new();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));
        let (circuit, nodes) = build_circuit();

        let table = RoutingTable::default();
        let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
        let mut writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        writer
            .add_circuit(circuit.circuit_id().to_string(), circuit, nodes)
            .expect("Unable to add circuit");
        let handler = ServiceConnectRequestHandler::new("123".to_string(), reader.clone(), writer);

        dispatcher.set_handler(Box::new(handler));
        let mut connect_request = ServiceConnectRequest::new();
        connect_request.set_circuit("alpha".into());
        connect_request.set_service_id("abc".into());
        let connect_bytes = connect_request.write_to_bytes().unwrap();

        dispatcher
            .dispatch(
                "abc".into(),
                &CircuitMessageType::SERVICE_CONNECT_REQUEST,
                connect_bytes.clone(),
            )
            .unwrap();

        let id = ServiceId::new("alpha".into(), "abc".into());
        assert!(reader.get_service(&id).unwrap().is_some());

        let (id, message) = mock_sender.next_outbound().expect("No message was sent");
        assert_network_message(
            message,
            id.into(),
            "abc",
            CircuitMessageType::SERVICE_CONNECT_RESPONSE,
            |msg: ServiceConnectResponse| {
                assert_eq!(msg.get_service_id(), "abc");
                assert_eq!(msg.get_circuit(), "alpha");
                assert_eq!(msg.get_status(), ServiceConnectResponse_Status::OK);
            },
        )
    }

    #[test]
    // Test that if the service is in a circuit and already connected, a ServiceConnectResponse is
    // returned with an ERROR_SERVICE_ALREADY_REGISTERED
    fn test_service_connect_request_handler_already_connected() {
        // Set up dispatcher and mock sender
        let mock_sender = MockSender::new();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));
        let (circuit, nodes) = build_circuit();

        let table = RoutingTable::default();
        let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
        let mut writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        writer
            .add_circuit(circuit.circuit_id().to_string(), circuit, nodes)
            .expect("Unable to add circuit");

        let id = ServiceId::new("alpha".into(), "abc".into());
        let mut service = reader
            .get_service(&id)
            .expect("Unable to get service")
            .unwrap();
        service.set_peer_id("abc_network".to_string());
        writer
            .add_service(id, service)
            .expect("Unable to add circuit");

        let handler = ServiceConnectRequestHandler::new("123".to_string(), reader, writer);

        dispatcher.set_handler(Box::new(handler));
        let mut connect_request = ServiceConnectRequest::new();
        connect_request.set_circuit("alpha".into());
        connect_request.set_service_id("abc".into());
        let connect_bytes = connect_request.write_to_bytes().unwrap();

        dispatcher
            .dispatch(
                "abc".into(),
                &CircuitMessageType::SERVICE_CONNECT_REQUEST,
                connect_bytes.clone(),
            )
            .unwrap();

        let (id, message) = mock_sender.next_outbound().expect("No message was sent");
        assert_network_message(
            message,
            id.into(),
            "abc",
            CircuitMessageType::SERVICE_CONNECT_RESPONSE,
            |msg: ServiceConnectResponse| {
                assert_eq!(msg.get_service_id(), "abc");
                assert_eq!(msg.get_circuit(), "alpha");
                assert_eq!(
                    msg.get_status(),
                    ServiceConnectResponse_Status::ERROR_SERVICE_ALREADY_REGISTERED
                );
            },
        )
    }

    #[test]
    // Test that if the circuit does not exist, a ServiceDisconnectResponse is returned with
    // a ERROR_CIRCUIT_DOES_NOT_EXIST
    fn test_service_disconnect_request_handler_no_circuit() {
        // Set up dispatcher and mock sender
        let mock_sender = MockSender::new();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));

        let table = RoutingTable::default();
        let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());
        let handler = ServiceDisconnectRequestHandler::new(reader, writer);

        dispatcher.set_handler(Box::new(handler));
        let mut disconnect_request = ServiceDisconnectRequest::new();
        disconnect_request.set_circuit("alpha".into());
        disconnect_request.set_service_id("abc".into());
        let disconnect_bytes = disconnect_request.write_to_bytes().unwrap();

        dispatcher
            .dispatch(
                "abc".into(),
                &CircuitMessageType::SERVICE_DISCONNECT_REQUEST,
                disconnect_bytes.clone(),
            )
            .unwrap();

        let (id, message) = mock_sender.next_outbound().expect("No message was sent");
        assert_network_message(
            message,
            id.into(),
            "abc",
            CircuitMessageType::SERVICE_DISCONNECT_RESPONSE,
            |msg: ServiceDisconnectResponse| {
                assert_eq!(msg.get_service_id(), "abc");
                assert_eq!(msg.get_circuit(), "alpha");
                assert_eq!(
                    msg.get_status(),
                    ServiceDisconnectResponse_Status::ERROR_CIRCUIT_DOES_NOT_EXIST
                );
            },
        )
    }

    #[test]
    // Test that if the service is not in circuit, a ServiceDisconnectResponse is returned with
    // a ERROR_SERVICE_NOT_IN_CIRCUIT_REGISTRY
    fn test_service_disconnect_request_handler_not_in_circuit() {
        // Set up dispatcher and mock sender
        let mock_sender = MockSender::new();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));

        let (circuit, nodes) = build_circuit();

        let table = RoutingTable::default();
        let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
        let mut writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        writer
            .add_circuit(circuit.circuit_id().to_string(), circuit, nodes)
            .expect("Unable to add circuit");

        let handler = ServiceDisconnectRequestHandler::new(reader, writer);

        dispatcher.set_handler(Box::new(handler));
        let mut disconnect_request = ServiceDisconnectRequest::new();
        disconnect_request.set_circuit("alpha".into());
        disconnect_request.set_service_id("BAD".into());
        let disconnect_bytes = disconnect_request.write_to_bytes().unwrap();

        dispatcher
            .dispatch(
                "BAD".into(),
                &CircuitMessageType::SERVICE_DISCONNECT_REQUEST,
                disconnect_bytes.clone(),
            )
            .unwrap();

        let (id, message) = mock_sender.next_outbound().expect("No message was sent");
        assert_network_message(
            message,
            id.into(),
            "BAD",
            CircuitMessageType::SERVICE_DISCONNECT_RESPONSE,
            |msg: ServiceDisconnectResponse| {
                assert_eq!(msg.get_service_id(), "BAD");
                assert_eq!(msg.get_circuit(), "alpha");
                assert_eq!(
                    msg.get_status(),
                    ServiceDisconnectResponse_Status::ERROR_SERVICE_NOT_IN_CIRCUIT_REGISTRY
                );
            },
        )
    }

    #[test]
    // Test that if the service is in a circuit and already connected, a ServiceDisconnectResponse
    // is returned with an OK.
    fn test_service_disconnect_request_handler() {
        // Set up dispatcher and mock sender
        let mock_sender = MockSender::new();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));

        let (circuit, nodes) = build_circuit();
        let table = RoutingTable::default();
        let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
        let mut writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        writer
            .add_circuit(circuit.circuit_id().to_string(), circuit, nodes)
            .expect("Unable to add circuit");

        let id = ServiceId::new("alpha".into(), "abc".into());
        let mut service = reader
            .get_service(&id)
            .expect("Unable to get service")
            .unwrap();
        service.set_peer_id("abc_network".to_string());
        writer
            .add_service(id, service)
            .expect("Unable to add circuit");

        let handler = ServiceDisconnectRequestHandler::new(reader, writer);

        dispatcher.set_handler(Box::new(handler));
        let mut disconnect_request = ServiceDisconnectRequest::new();
        disconnect_request.set_circuit("alpha".into());
        disconnect_request.set_service_id("abc".into());
        let disconnect_bytes = disconnect_request.write_to_bytes().unwrap();

        dispatcher
            .dispatch(
                "abc".into(),
                &CircuitMessageType::SERVICE_DISCONNECT_REQUEST,
                disconnect_bytes.clone(),
            )
            .unwrap();

        let (id, message) = mock_sender.next_outbound().expect("No message was sent");
        assert_network_message(
            message,
            id.into(),
            "abc",
            CircuitMessageType::SERVICE_DISCONNECT_RESPONSE,
            |msg: ServiceDisconnectResponse| {
                assert_eq!(msg.get_service_id(), "abc");
                assert_eq!(msg.get_circuit(), "alpha");
                assert_eq!(msg.get_status(), ServiceDisconnectResponse_Status::OK);
            },
        )
    }

    #[test]
    // Test that if the service is in a circuit and not connected, a ServiceDisconnectResponse
    // is returned with an ERROR_SERVICE_NOT_REGISTERED
    fn test_service_disconnect_request_handler_not_connected() {
        // Set up dispatcher and mock sender
        let mock_sender = MockSender::new();
        let mut dispatcher = Dispatcher::new(Box::new(mock_sender.clone()));

        let (circuit, nodes) = build_circuit();
        let table = RoutingTable::default();
        let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
        let mut writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        writer
            .add_circuit(circuit.circuit_id().to_string(), circuit, nodes)
            .expect("Unable to add circuit");

        let handler = ServiceDisconnectRequestHandler::new(reader, writer);

        dispatcher.set_handler(Box::new(handler));
        let mut disconnect_request = ServiceDisconnectRequest::new();
        disconnect_request.set_circuit("alpha".into());
        disconnect_request.set_service_id("abc".into());
        let disconnect_bytes = disconnect_request.write_to_bytes().unwrap();

        dispatcher
            .dispatch(
                "abc".into(),
                &CircuitMessageType::SERVICE_DISCONNECT_REQUEST,
                disconnect_bytes.clone(),
            )
            .unwrap();

        let (id, message) = mock_sender.next_outbound().expect("No message was sent");
        assert_network_message(
            message,
            id.into(),
            "abc",
            CircuitMessageType::SERVICE_DISCONNECT_RESPONSE,
            |msg: ServiceDisconnectResponse| {
                assert_eq!(msg.get_service_id(), "abc");
                assert_eq!(msg.get_circuit(), "alpha");
                assert_eq!(
                    msg.get_status(),
                    ServiceDisconnectResponse_Status::ERROR_SERVICE_NOT_REGISTERED
                );
            },
        )
    }

    fn build_circuit() -> (Circuit, Vec<CircuitNode>) {
        let node_123 = CircuitNode::new("123".to_string(), vec!["123.0.0.1:0".to_string()]);
        let node_345 = CircuitNode::new("345".to_string(), vec!["123.0.0.1:1".to_string()]);

        let service_abc = Service::new(
            "abc".to_string(),
            "test".to_string(),
            "123".to_string(),
            vec![],
        );
        let service_def = Service::new(
            "def".to_string(),
            "test".to_string(),
            "345".to_string(),
            vec![],
        );

        // Add circuit and service to splinter state
        let circuit = Circuit::new(
            "alpha".into(),
            vec![service_abc.clone(), service_def.clone()],
            vec!["123".into(), "345".into()],
        );

        (circuit, vec![node_123, node_345])
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
