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
use crate::circuit::service::{Service, ServiceId, SplinterNode};
use crate::circuit::{ServiceDefinition, SplinterState};
use crate::network::dispatch::{DispatchError, Handler, MessageContext, MessageSender, PeerId};
use crate::protos::circuit::{
    CircuitMessageType, ServiceConnectRequest, ServiceConnectResponse,
    ServiceConnectResponse_Status, ServiceDisconnectRequest, ServiceDisconnectResponse,
    ServiceDisconnectResponse_Status,
};

use protobuf::Message;

// Implements a handler that handles ServiceConnectRequest
pub struct ServiceConnectRequestHandler {
    node_id: String,
    endpoints: Vec<String>,
    state: SplinterState,
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
            .state
            .circuit(circuit_name)
            .map_err(|err| DispatchError::HandleError(err.context()))?;

        if let Some(circuit) = circuit_result {
            // If the circuit has the service in its roster and the service is not yet connected
            // forward the connection to the rest of the nodes on the circuit and add the service
            // to splinter state
            if circuit.roster().contains(&service_id.to_string())
                && !self
                    .state
                    .has_service(&unique_id)
                    .map_err(|err| DispatchError::HandleError(err.context()))?
            {
                // This should never return None since we just checked if it exists.
                // If admin service create a service defination for the admin service
                let service = {
                    if !service_id.starts_with("admin::") {
                        circuit
                            .roster()
                            .iter()
                            .find(|service| service.service_id == service_id)
                            .expect("Cannot find service in circuit")
                            .clone()
                    } else {
                        ServiceDefinition::builder(service_id.into(), "admin".into())
                            .with_allowed_nodes(vec![self.node_id.to_string()])
                            .build()
                    }
                };

                if !service.allowed_nodes.contains(&self.node_id) {
                    response.set_status(ServiceConnectResponse_Status::ERROR_NOT_AN_ALLOWED_NODE);
                    response.set_error_message(format!("{} is not allowed on this node", unique_id))
                } else {
                    let node = SplinterNode::new(self.node_id.to_string(), self.endpoints.to_vec());
                    let service = Service::new(
                        service_id.to_string(),
                        Some(context.source_peer_id().to_string()),
                        node,
                    );
                    self.state
                        .add_service(unique_id, service)
                        .map_err(|err| DispatchError::HandleError(err.context()))?;

                    response.set_status(ServiceConnectResponse_Status::OK);
                }
            // If the circuit exists and has the service in the roster but the service is already
            // connected, return an error response
            } else if circuit.roster().contains(&service_id.to_string())
                && self
                    .state
                    .has_service(&unique_id)
                    .map_err(|err| DispatchError::HandleError(err.context()))?
            {
                response
                    .set_status(ServiceConnectResponse_Status::ERROR_SERVICE_ALREADY_REGISTERED);
                response.set_error_message(format!("Service is already registered: {}", service_id))
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
    pub fn new(node_id: String, endpoints: Vec<String>, state: SplinterState) -> Self {
        ServiceConnectRequestHandler {
            node_id,
            endpoints,
            state,
        }
    }
}

// Implements a handler that handles ServiceDisconnectRequest
pub struct ServiceDisconnectRequestHandler {
    state: SplinterState,
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
            .state
            .circuit(circuit_name)
            .map_err(|err| DispatchError::HandleError(err.context()))?;

        if let Some(circuit) = circuit_result {
            // If the circuit has the service in its roster and the service is connected
            // forward the disconnection to the rest of the nodes on the circuit and remove the
            // service from splinter state
            if circuit.roster().contains(&service_id.to_string())
                && self
                    .state
                    .has_service(&unique_id)
                    .map_err(|err| DispatchError::HandleError(err.context()))?
            {
                self.state
                    .remove_service(&unique_id)
                    .map_err(|err| DispatchError::HandleError(err.context()))?;
                response.set_status(ServiceDisconnectResponse_Status::OK);
            // If the circuit exists and has the service in the roster but the service not
            // connected, return an error response
            } else if circuit.roster().contains(&service_id.to_string())
                && !self
                    .state
                    .has_service(&unique_id)
                    .map_err(|err| DispatchError::HandleError(err.context()))?
            {
                response.set_status(ServiceDisconnectResponse_Status::ERROR_SERVICE_NOT_REGISTERED);
                response.set_error_message(format!("Service is not registered: {}", service_id))
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
    pub fn new(state: SplinterState) -> Self {
        ServiceDisconnectRequestHandler { state }
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
    use crate::circuit::directory::CircuitDirectory;
    use crate::circuit::{AuthorizationType, Circuit, DurabilityType, PersistenceType, RouteType};
    use crate::mesh::Mesh;
    use crate::network::dispatch::Dispatcher;
    use crate::network::sender;
    use crate::network::{Network, NetworkMessageWrapper};
    use crate::protos::circuit::CircuitMessage;
    use crate::protos::network::NetworkMessage;
    use crate::storage::get_storage;
    use crate::transport::inproc::InprocTransport;
    use crate::transport::{Listener, Transport};

    #[test]
    // Test that if the circuit does not exist, a ServiceConnectResponse is returned with
    // a ERROR_CIRCUIT_DOES_NOT_EXIST
    fn test_service_connect_request_handler_no_circuit() {
        run_test(
            |mut listener, mut dispatcher, network1| {
                let connection = listener.accept().expect("Cannot accept connection");
                network1
                    .add_peer("abc".to_string(), connection)
                    .expect("Unable to add peer");

                let storage = get_storage("memory", CircuitDirectory::new).unwrap();
                let circuit_directory = storage.read().clone();
                let state = SplinterState::new("memory".to_string(), circuit_directory);
                let handler = ServiceConnectRequestHandler::new(
                    "123".to_string(),
                    vec!["127.0.0.1:0".to_string()],
                    state,
                );

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
            },
            "123",
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
        run_test(
            |mut listener, mut dispatcher, network1| {
                let connection = listener.accept().expect("Cannot accept connection");
                network1
                    .add_peer("BAD".to_string(), connection)
                    .expect("Unable to add peer");

                let circuit = build_circuit();

                let mut circuit_directory = CircuitDirectory::new();
                circuit_directory.add_circuit("alpha".to_string(), circuit);

                let state = SplinterState::new("memory".to_string(), circuit_directory);
                let handler = ServiceConnectRequestHandler::new(
                    "123".to_string(),
                    vec!["127.0.0.1:0".to_string()],
                    state,
                );

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
            },
            "123",
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
        run_test(
            |mut listener, mut dispatcher, network1| {
                let connection = listener.accept().expect("Cannot accept connection");
                network1
                    .add_peer("abc".to_string(), connection)
                    .expect("Unable to add peer");

                let circuit = build_circuit();

                let mut circuit_directory = CircuitDirectory::new();
                circuit_directory.add_circuit("alpha".to_string(), circuit);

                let state = SplinterState::new("memory".to_string(), circuit_directory);
                let handler = ServiceConnectRequestHandler::new(
                    "123".to_string(),
                    vec!["127.0.0.1:0".to_string()],
                    state.clone(),
                );

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
                assert!(state.get_service(&id).unwrap().is_some());
            },
            "123",
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
        run_test(
            |mut listener, mut dispatcher, network1| {
                let connection = listener.accept().expect("Cannot accept connection");
                network1
                    .add_peer("abc".to_string(), connection)
                    .expect("Unable to add peer");

                let circuit = build_circuit();

                let mut circuit_directory = CircuitDirectory::new();
                circuit_directory.add_circuit("alpha".to_string(), circuit);

                let state = SplinterState::new("memory".to_string(), circuit_directory);

                let node = SplinterNode::new("123".to_string(), vec!["123.0.0.1:0".to_string()]);
                let service =
                    Service::new("abc".to_string(), Some("abc_network".to_string()), node);
                let id = ServiceId::new("alpha".into(), "abc".into());
                state.add_service(id.clone(), service).unwrap();
                let handler = ServiceConnectRequestHandler::new(
                    "123".to_string(),
                    vec!["127.0.0.1:0".to_string()],
                    state,
                );

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
            },
            "123",
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
        run_test(
            |mut listener, mut dispatcher, network1| {
                let connection = listener.accept().expect("Cannot accept connection");
                network1
                    .add_peer("abc".to_string(), connection)
                    .expect("Unable to add peer");

                let storage = get_storage("memory", CircuitDirectory::new).unwrap();
                let circuit_directory = storage.read().clone();
                let state = SplinterState::new("memory".to_string(), circuit_directory);
                let handler = ServiceDisconnectRequestHandler::new(state);

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
            },
            "123",
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
        run_test(
            |mut listener, mut dispatcher, network1| {
                let connection = listener.accept().expect("Cannot accept connection");
                network1
                    .add_peer("BAD".to_string(), connection)
                    .expect("Unable to add peer");

                let circuit = build_circuit();

                let mut circuit_directory = CircuitDirectory::new();
                circuit_directory.add_circuit("alpha".to_string(), circuit);

                let state = SplinterState::new("memory".to_string(), circuit_directory);
                let handler = ServiceDisconnectRequestHandler::new(state);

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
            },
            "123",
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
        run_test(
            |mut listener, mut dispatcher, network1| {
                let connection = listener.accept().expect("Cannot accept connection");
                network1
                    .add_peer("abc".to_string(), connection)
                    .expect("Unable to add peer");

                let circuit = build_circuit();

                let mut circuit_directory = CircuitDirectory::new();
                circuit_directory.add_circuit("alpha".to_string(), circuit);

                let state = SplinterState::new("memory".to_string(), circuit_directory);

                let node = SplinterNode::new("123".to_string(), vec!["123.0.0.1:0".to_string()]);
                let service =
                    Service::new("abc".to_string(), Some("abc_network".to_string()), node);
                let id = ServiceId::new("alpha".into(), "abc".into());
                state.add_service(id.clone(), service).unwrap();

                let handler = ServiceDisconnectRequestHandler::new(state.clone());

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
            },
            "123",
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
        run_test(
            |mut listener, mut dispatcher, network1| {
                let connection = listener.accept().expect("Cannot accept connection");
                network1
                    .add_peer("abc".to_string(), connection)
                    .expect("Unable to add peer");

                let circuit = build_circuit();

                let mut circuit_directory = CircuitDirectory::new();
                circuit_directory.add_circuit("alpha".to_string(), circuit);

                let state = SplinterState::new("memory".to_string(), circuit_directory);

                let handler = ServiceDisconnectRequestHandler::new(state);

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
            },
            "123",
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

    fn build_circuit() -> Circuit {
        let service_abc = ServiceDefinition::builder("abc".into(), "test".into())
            .with_allowed_nodes(vec!["123".to_string()])
            .build();

        let service_def = ServiceDefinition::builder("def".into(), "test".into())
            .with_allowed_nodes(vec!["345".to_string()])
            .build();

        let circuit = Circuit::builder()
            .with_id("alpha".into())
            .with_auth(AuthorizationType::Trust)
            .with_members(vec!["123".into(), "345".into()])
            .with_roster(vec![service_abc, service_def])
            .with_persistence(PersistenceType::Any)
            .with_durability(DurabilityType::NoDurability)
            .with_routes(RouteType::Any)
            .with_circuit_management_type("service_connect_test_app".into())
            .build()
            .expect("Should have built a correct circuit");

        circuit
    }

    // Helper function for running the tests. This function starts up two networks, a transport
    // and a dispatcher. The function is passed the test to run, and the expected message that
    // will be should be returned.
    fn run_test<F: 'static, M: protobuf::Message, A>(
        test: F,
        expected_sender: &str,
        expected_circuit_msg_type: CircuitMessageType,
        detail_assertions: A,
    ) where
        F: Fn(Box<dyn Listener>, Dispatcher<CircuitMessageType>, Network) -> () + Send,
        A: Fn(M),
    {
        // Set up dispatcher and mock sender
        let mesh1 = Mesh::new(1, 1);
        let network1 = Network::new(mesh1.clone(), 0).unwrap();

        let network_message_queue = sender::Builder::new()
            .with_network(network1.clone())
            .build()
            .expect("Unable to create queue");
        let network_sender = network_message_queue.new_network_sender();

        let mut inproc_transport = InprocTransport::default();
        let dispatcher = Dispatcher::new(network_sender);
        let listener = inproc_transport
            .listen("inproc://service_handler")
            .expect("Cannot get listener");

        std::thread::spawn(move || test(listener, dispatcher, network1));

        let mesh2 = Mesh::new(1, 1);
        let network2 = Network::new(mesh2.clone(), 0).unwrap();
        let connection = inproc_transport
            .connect("inproc://service_handler")
            .expect("Unable to connect to inproc");
        network2
            .add_peer("123".to_string(), connection)
            .expect("Unable to add peer");
        let network_message = network2
            .recv()
            .expect("Unable to receive message over the network");

        assert_network_message(
            network_message,
            expected_sender,
            expected_circuit_msg_type,
            detail_assertions,
        )
    }

    fn assert_network_message<M: protobuf::Message, F: Fn(M)>(
        network_message: NetworkMessageWrapper,
        expected_sender: &str,
        expected_circuit_msg_type: CircuitMessageType,
        detail_assertions: F,
    ) {
        assert_eq!(expected_sender, network_message.peer_id());

        let network_msg: NetworkMessage =
            protobuf::parse_from_bytes(network_message.payload()).unwrap();
        let circuit_msg: CircuitMessage =
            protobuf::parse_from_bytes(network_msg.get_payload()).unwrap();
        assert_eq!(expected_circuit_msg_type, circuit_msg.get_message_type(),);
        let circuit_msg: M = protobuf::parse_from_bytes(circuit_msg.get_payload()).unwrap();

        detail_assertions(circuit_msg);
    }
}
