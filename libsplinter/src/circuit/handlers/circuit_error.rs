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
use crate::circuit::{ServiceId, SplinterState};
use crate::network::dispatch::{DispatchError, Handler, MessageContext, MessageSender, PeerId};
use crate::protos::circuit::{CircuitError, CircuitMessageType};

// Implements a handler that handles CircuitError messages
pub struct CircuitErrorHandler {
    node_id: String,
    state: SplinterState,
}

// In most cases the error message will be returned directly back to service, but in the case
// where it is returned back to a different node, this node will do its best effort to
// return it back to the service or node who sent the original message.
impl Handler for CircuitErrorHandler {
    type Source = PeerId;
    type MessageType = CircuitMessageType;
    type Message = CircuitError;

    fn match_type(&self) -> Self::MessageType {
        CircuitMessageType::CIRCUIT_ERROR_MESSAGE
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        debug!("Handle Circuit Error Message {:?}", msg);
        let circuit_name = msg.get_circuit_name();
        let service_id = msg.get_service_id();
        let unique_id = ServiceId::new(circuit_name.to_string(), service_id.to_string());

        // check if the msg_sender is in the service directory
        let recipient = match self
            .state
            .get_service(&unique_id)
            .map_err(|err| DispatchError::HandleError(err.context()))?
        {
            Some(service) => {
                let node_id = service.node().id();
                if node_id == self.node_id {
                    // If the service is connected to this node, send the error to the service
                    match service.peer_id() {
                        Some(peer_id) => peer_id.to_string(),
                        None => {
                            // This should never happen, as a peer id will always
                            // be set on a service that is connected to the local node.
                            warn!("No peer id for service:{} ", service.service_id());
                            return Ok(());
                        }
                    }
                } else {
                    // If the service is connected to another node, send the error to that node
                    service.node().id().to_string()
                }
            }
            None => {
                // If the service is not in the service directory, the nodes does not know who to
                // forward this message to, so the message is dropped
                warn!(
                    "Original message sender is not connected: {}, cannot send Circuit Error",
                    service_id
                );
                return Ok(());
            }
        };

        let network_msg_bytes = create_message(
            context.message_bytes().to_vec(),
            CircuitMessageType::CIRCUIT_ERROR_MESSAGE,
        )?;

        // forward error message
        sender
            .send(recipient.into(), network_msg_bytes)
            .map_err(|(recipient, payload)| {
                DispatchError::NetworkSendError((recipient.into(), payload))
            })?;
        Ok(())
    }
}

impl CircuitErrorHandler {
    pub fn new(node_id: String, state: SplinterState) -> Self {
        CircuitErrorHandler { node_id, state }
    }
}

#[cfg(test)]
mod tests {
    use protobuf::Message;

    use super::*;
    use crate::circuit::directory::CircuitDirectory;
    use crate::circuit::service::{Service, SplinterNode};
    use crate::circuit::{AuthorizationType, Circuit, DurabilityType, PersistenceType, RouteType};
    use crate::mesh::Mesh;
    use crate::network::dispatch::Dispatcher;
    use crate::network::sender;
    use crate::network::{Network, NetworkMessageWrapper};
    use crate::protos::circuit::{CircuitError_Error, CircuitMessage};
    use crate::protos::network::{NetworkEcho, NetworkMessage, NetworkMessageType};
    use crate::transport::inproc::InprocTransport;
    use crate::transport::{Listener, Transport};

    // Test that if an error message received is meant for the service connected to a node,
    // the error message is sent to the service
    #[test]
    fn test_circuit_error_handler_service() {
        // Set up disptacher and mock sender
        run_test(
            |mut listener, mut dispatcher, network1| {
                let connection = listener.accept().expect("Cannot accept connection");
                network1
                    .add_peer("abc_network".to_string(), connection)
                    .expect("Unable to add peer");

                // Add circuit and service to splinter state
                let circuit = Circuit::builder()
                    .with_id("alpha".into())
                    .with_auth(AuthorizationType::Trust)
                    .with_members(vec!["123".into()])
                    .with_roster(vec!["abc".into(), "def".into()])
                    .with_persistence(PersistenceType::Any)
                    .with_durability(DurabilityType::NoDurability)
                    .with_routes(RouteType::Any)
                    .with_circuit_management_type("circuit_errors_test_app".into())
                    .build()
                    .expect("Should have built a correct circuit");

                let mut circuit_directory = CircuitDirectory::new();
                circuit_directory.add_circuit("alpha".to_string(), circuit);

                let state = SplinterState::new("memory".to_string(), circuit_directory);

                let node_123 =
                    SplinterNode::new("123".to_string(), vec!["123.0.0.1:0".to_string()]);
                let node_345 =
                    SplinterNode::new("345".to_string(), vec!["123.0.0.1:1".to_string()]);

                let service_abc =
                    Service::new("abc".to_string(), Some("abc_network".to_string()), node_123);
                let service_def =
                    Service::new("def".to_string(), Some("def_network".to_string()), node_345);

                let abc_id = ServiceId::new("alpha".into(), "abc".into());
                let def_id = ServiceId::new("alpha".into(), "def".into());
                state.add_service(abc_id, service_abc).unwrap();
                state.add_service(def_id, service_def).unwrap();

                // Add circuit error handler to the the dispatcher
                let handler = CircuitErrorHandler::new("123".to_string(), state);
                dispatcher.set_handler(Box::new(handler));

                // Create the error message
                let mut circuit_error = CircuitError::new();
                circuit_error.set_service_id("abc".into());
                circuit_error.set_circuit_name("alpha".into());
                circuit_error.set_correlation_id("1234".into());
                circuit_error.set_error(CircuitError_Error::ERROR_RECIPIENT_NOT_IN_DIRECTORY);
                circuit_error.set_error_message("TEST".into());
                let error_bytes = circuit_error.write_to_bytes().unwrap();

                // dispatch the error message
                dispatcher
                    .dispatch(
                        "345".into(),
                        &CircuitMessageType::CIRCUIT_ERROR_MESSAGE,
                        error_bytes.clone(),
                    )
                    .unwrap();
            },
            "123",
            CircuitMessageType::CIRCUIT_ERROR_MESSAGE,
            |msg: CircuitError| {
                assert_eq!(msg.get_service_id(), "abc");
                assert_eq!(msg.get_circuit_name(), "alpha");
                assert_eq!(
                    msg.get_error(),
                    CircuitError_Error::ERROR_RECIPIENT_NOT_IN_DIRECTORY
                );
                assert_eq!(msg.get_error_message(), "TEST");
                assert_eq!(msg.get_correlation_id(), "1234");
            },
        )
    }

    // Test that if an error message received is meant for the service not connected to this node,
    // the error message is sent to the node the service is connected to
    #[test]
    fn test_circuit_error_handler_node() {
        // Set up disptacher and mock sender
        run_test(
            |mut listener, mut dispatcher, network1| {
                let connection = listener.accept().expect("Cannot accept connection");
                network1
                    .add_peer("345".to_string(), connection)
                    .expect("Unable to add peer");

                // Add circuit and service to splinter state
                let circuit = Circuit::builder()
                    .with_id("alpha".into())
                    .with_auth(AuthorizationType::Trust)
                    .with_members(vec!["123".into()])
                    .with_roster(vec!["abc".into(), "def".into()])
                    .with_persistence(PersistenceType::Any)
                    .with_durability(DurabilityType::NoDurability)
                    .with_routes(RouteType::Any)
                    .with_circuit_management_type("circuit_error_test_app".into())
                    .build()
                    .expect("Should have built a correct circuit");

                let mut circuit_directory = CircuitDirectory::new();
                circuit_directory.add_circuit("alpha".to_string(), circuit);

                let state = SplinterState::new("memory".to_string(), circuit_directory);

                let node_123 =
                    SplinterNode::new("123".to_string(), vec!["123.0.0.1:0".to_string()]);
                let node_345 =
                    SplinterNode::new("345".to_string(), vec!["123.0.0.1:1".to_string()]);

                let service_abc =
                    Service::new("abc".to_string(), Some("abc_network".to_string()), node_123);
                let service_def =
                    Service::new("def".to_string(), Some("def_network".to_string()), node_345);

                let abc_id = ServiceId::new("alpha".into(), "abc".into());
                let def_id = ServiceId::new("alpha".into(), "def".into());
                state.add_service(abc_id, service_abc).unwrap();
                state.add_service(def_id, service_def).unwrap();

                // Add circuit error handler to the the dispatcher
                let handler = CircuitErrorHandler::new("123".to_string(), state);
                dispatcher.set_handler(Box::new(handler));

                // Create the error message
                let mut circuit_error = CircuitError::new();
                circuit_error.set_service_id("def".into());
                circuit_error.set_circuit_name("alpha".into());
                circuit_error.set_correlation_id("1234".into());
                circuit_error.set_error(CircuitError_Error::ERROR_RECIPIENT_NOT_IN_DIRECTORY);
                circuit_error.set_error_message("TEST".into());
                let error_bytes = circuit_error.write_to_bytes().unwrap();

                // dispatch the error message
                dispatcher
                    .dispatch(
                        "586".into(),
                        &CircuitMessageType::CIRCUIT_ERROR_MESSAGE,
                        error_bytes.clone(),
                    )
                    .unwrap();
            },
            "123",
            CircuitMessageType::CIRCUIT_ERROR_MESSAGE,
            |msg: CircuitError| {
                assert_eq!(msg.get_service_id(), "def");
                assert_eq!(
                    msg.get_error(),
                    CircuitError_Error::ERROR_RECIPIENT_NOT_IN_DIRECTORY
                );
                assert_eq!(msg.get_error_message(), "TEST");
                assert_eq!(msg.get_correlation_id(), "1234");
            },
        )
    }

    // Test that if the service the error message is meant for is not connected, the message is
    // dropped because there is no way to know where to send it. This test sends NetworkEcho
    #[test]
    fn test_circuit_error_handler_no_service() {
        // Set up disptacher and mock sender
        // Set up dispatcher and mock sender
        let mesh1 = Mesh::new(1, 1);
        let network1 = Network::new(mesh1.clone(), 0).unwrap();

        let network_message_queue = sender::Builder::new()
            .with_network(network1.clone())
            .build()
            .expect("Unable to create queue");
        let network_sender = network_message_queue.new_network_sender();
        let network_echo_sender = network_message_queue.new_network_sender();

        let mut inproc_transport = InprocTransport::default();
        let mut dispatcher = Dispatcher::new(network_sender);
        let mut listener = inproc_transport
            .listen("inproc://circuit_error")
            .expect("Cannot get listener");

        std::thread::spawn(move || {
            let connection = listener.accept().expect("Cannot accept connection");
            network1
                .add_peer("345".to_string(), connection)
                .expect("Unable to add peer");
            // create empty state
            let circuit_directory = CircuitDirectory::new();

            let state = SplinterState::new("memory".to_string(), circuit_directory);

            // Add circuit error handler to the the dispatcher
            let handler = CircuitErrorHandler::new("123".to_string(), state);
            dispatcher.set_handler(Box::new(handler));

            // Create the circuit error message
            let mut circuit_error = CircuitError::new();
            circuit_error.set_service_id("abc".into());
            circuit_error.set_circuit_name("alpha".into());
            circuit_error.set_correlation_id("1234".into());
            circuit_error.set_error(CircuitError_Error::ERROR_RECIPIENT_NOT_IN_DIRECTORY);
            circuit_error.set_error_message("TEST".into());
            let error_bytes = circuit_error.write_to_bytes().unwrap();

            // dispatch the error message
            dispatcher
                .dispatch(
                    "def".into(),
                    &CircuitMessageType::CIRCUIT_ERROR_MESSAGE,
                    error_bytes.clone(),
                )
                .unwrap();

            let mut network_echo = NetworkEcho::new();
            network_echo.set_payload(b"send_echo".to_vec());
            let mut network_msg = NetworkMessage::new();
            network_msg.set_payload(network_echo.write_to_bytes().unwrap());
            network_msg.set_message_type(NetworkMessageType::NETWORK_ECHO);
            network_echo_sender
                .send("345".to_string(), network_msg.write_to_bytes().unwrap())
                .expect("Unable to send network echo");
        });

        let mesh2 = Mesh::new(1, 1);
        let network2 = Network::new(mesh2.clone(), 0).unwrap();
        let connection = inproc_transport
            .connect("inproc://circuit_error")
            .expect("Unable to connect to inproc");
        network2
            .add_peer("123".to_string(), connection)
            .expect("Unable to add peer");
        let network_message = network2
            .recv()
            .expect("Unable to receive message over the network");

        // verify that the message returned was an NetworkEcho, not a CircuitError
        let network_msg: NetworkMessage =
            protobuf::parse_from_bytes(network_message.payload()).unwrap();

        assert_eq!(
            network_msg.get_message_type(),
            NetworkMessageType::NETWORK_ECHO
        );
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
            .listen("inproc://circuit_error")
            .expect("Cannot get listener");

        std::thread::spawn(move || test(listener, dispatcher, network1));

        let mesh2 = Mesh::new(1, 1);
        let network2 = Network::new(mesh2.clone(), 0).unwrap();
        let connection = inproc_transport
            .connect("inproc://circuit_error")
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
