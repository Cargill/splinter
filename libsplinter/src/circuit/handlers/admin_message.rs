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
use crate::circuit::SplinterState;
use crate::network::dispatch::{DispatchError, Handler, MessageContext, MessageSender, PeerId};
use crate::protos::circuit::{
    AdminDirectMessage, CircuitError, CircuitError_Error, CircuitMessageType,
};
use protobuf::Message;

const ADMIN_SERVICE_ID_PREFIX: &str = "admin::";

// Implements a handler that handles AdminDirectMessage
pub struct AdminDirectMessageHandler {
    node_id: String,
    state: SplinterState,
}

impl Handler for AdminDirectMessageHandler {
    type Source = PeerId;
    type MessageType = CircuitMessageType;
    type Message = AdminDirectMessage;

    fn match_type(&self) -> Self::MessageType {
        CircuitMessageType::ADMIN_DIRECT_MESSAGE
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        debug!(
            "Handle Admin Direct Message {} on {} ({} => {}) [{} byte{}]",
            msg.get_correlation_id(),
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

        // msg bytes will either be message bytes of a direct message or an error message
        // the msg_recipient is either the service/node id to send the message to or is the
        // peer_id to send back the error message
        let (msg_bytes, msg_recipient) = self.create_response(msg, context)?;
        // either forward the direct message or send back an error message.
        sender
            .send(msg_recipient.into(), msg_bytes)
            .map_err(|(recipient, payload)| {
                DispatchError::NetworkSendError((recipient.into(), payload))
            })?;
        Ok(())
    }
}

impl AdminDirectMessageHandler {
    pub fn new(node_id: String, state: SplinterState) -> Self {
        Self { node_id, state }
    }

    fn create_response(
        &self,
        msg: AdminDirectMessage,
        context: &MessageContext<PeerId, CircuitMessageType>,
    ) -> Result<(Vec<u8>, String), DispatchError> {
        let circuit_name = msg.get_circuit();
        let msg_sender = msg.get_sender();
        let recipient = msg.get_recipient();

        if !is_admin_service_id(msg_sender) {
            let err_msg_bytes = create_circuit_error_msg(
                &msg,
                CircuitError_Error::ERROR_SENDER_NOT_IN_CIRCUIT_ROSTER,
                format!(
                    "Sender is not allowed to send admin messages: {}",
                    msg_sender
                ),
            )?;
            return Ok((
                create_message(err_msg_bytes, CircuitMessageType::CIRCUIT_ERROR_MESSAGE)?,
                context.source_peer_id().into(),
            ));
        }

        if !is_admin_service_id(recipient) {
            let err_msg_bytes = create_circuit_error_msg(
                &msg,
                CircuitError_Error::ERROR_RECIPIENT_NOT_IN_CIRCUIT_ROSTER,
                format!(
                    "Recipient is not allowed to receive admin messages: {}",
                    recipient
                ),
            )?;
            return Ok((
                create_message(err_msg_bytes, CircuitMessageType::CIRCUIT_ERROR_MESSAGE)?,
                context.source_peer_id().into(),
            ));
        }

        // msg bytes will either be message bytes of a direct message or an error message
        // the msg_recipient is either the service/node id to send the message to or is the
        // peer_id to send back the error message
        let circuit = self
            .state
            .circuit(circuit_name)
            .map_err(|err| DispatchError::HandleError(err.context()))?;

        let response = if circuit.is_some() {
            let node_id = &recipient[ADMIN_SERVICE_ID_PREFIX.len()..];
            // If the service is on this node send message to the service, otherwise
            // send the message to the node the service is connected to
            let target_node = if node_id != self.node_id {
                node_id
            } else {
                // The internal admin service is at the node id with an identical name
                recipient
            };

            let msg_bytes = context.message_bytes().to_vec();
            let network_msg_bytes =
                create_message(msg_bytes, CircuitMessageType::ADMIN_DIRECT_MESSAGE)?;
            (network_msg_bytes, target_node.to_string())
        } else {
            // if the circuit does not exist, send circuit error
            let msg_bytes = create_circuit_error_msg(
                &msg,
                CircuitError_Error::ERROR_CIRCUIT_DOES_NOT_EXIST,
                format!("Circuit does not exist: {}", circuit_name),
            )?;

            let network_msg_bytes =
                create_message(msg_bytes, CircuitMessageType::CIRCUIT_ERROR_MESSAGE)?;
            (network_msg_bytes, context.source_peer_id().to_string())
        };
        Ok(response)
    }
}

fn create_circuit_error_msg(
    msg: &AdminDirectMessage,
    error_type: CircuitError_Error,
    error_msg: String,
) -> Result<Vec<u8>, DispatchError> {
    let mut error_message = CircuitError::new();
    error_message.set_correlation_id(msg.get_correlation_id().into());
    error_message.set_service_id(msg.get_sender().into());
    error_message.set_circuit_name(msg.get_circuit().into());
    error_message.set_error(error_type);
    error_message.set_error_message(error_msg);

    error_message.write_to_bytes().map_err(DispatchError::from)
}

fn is_admin_service_id(service_id: &str) -> bool {
    service_id.starts_with(ADMIN_SERVICE_ID_PREFIX)
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
    use crate::transport::inproc::InprocTransport;
    use crate::transport::{Listener, Transport};

    /// Send a message from a non-admin service. Expect that the message is ignored and an error
    /// is returned to sender.
    #[test]
    fn test_ignore_non_admin_sender() {
        run_test(
            |mut listener, mut dispatcher, network1| {
                let connection = listener.accept().expect("Cannot accept connection");
                network1
                    .add_peer("5678".to_string(), connection)
                    .expect("Unable to add peer");

                // Add circuit and service to splinter state
                let circuit = Circuit::builder()
                    .with_id("alpha".into())
                    .with_auth(AuthorizationType::Trust)
                    .with_members(vec!["1234".into(), "5678".into()])
                    .with_roster(vec!["abc".into(), "def".into()])
                    .with_persistence(PersistenceType::Any)
                    .with_durability(DurabilityType::NoDurability)
                    .with_routes(RouteType::Any)
                    .with_circuit_management_type("admin_test_app".into())
                    .build()
                    .expect("Should have built a correct circuit");

                let mut circuit_directory = CircuitDirectory::new();
                circuit_directory.add_circuit("alpha".to_string(), circuit);

                let state = SplinterState::new("memory".to_string(), circuit_directory);

                let handler = AdminDirectMessageHandler::new("1234".into(), state);
                dispatcher.set_handler(Box::new(handler));

                let mut direct_message = AdminDirectMessage::new();
                direct_message.set_circuit("admin".into());
                direct_message.set_sender("abc".into());
                direct_message.set_recipient("admin::1234".into());
                direct_message.set_payload(b"test".to_vec());
                direct_message.set_correlation_id("random_corr_id".into());
                let direct_bytes = direct_message.write_to_bytes().unwrap();

                assert_eq!(
                    Ok(()),
                    dispatcher.dispatch(
                        "5678".into(),
                        &CircuitMessageType::ADMIN_DIRECT_MESSAGE,
                        direct_bytes
                    )
                );
            },
            "1234",
            CircuitMessageType::CIRCUIT_ERROR_MESSAGE,
            |error_msg: CircuitError| {
                assert_eq!(error_msg.get_service_id(), "abc");
                assert_eq!(
                    error_msg.get_error(),
                    CircuitError_Error::ERROR_SENDER_NOT_IN_CIRCUIT_ROSTER
                );
                assert_eq!(error_msg.get_correlation_id(), "random_corr_id");
            },
        )
    }

    /// Send a message to a non-admin service. Expect that the message is ignored and an error is
    /// returned to sender.
    #[test]
    fn test_ignore_non_admin_recipient() {
        run_test(
            |mut listener, mut dispatcher, network1| {
                let connection = listener.accept().expect("Cannot accept connection");
                network1
                    .add_peer("5678".to_string(), connection)
                    .expect("Unable to add peer");
                // Add circuit and service to splinter state
                let circuit = Circuit::builder()
                    .with_id("alpha".into())
                    .with_auth(AuthorizationType::Trust)
                    .with_members(vec!["1234".into(), "5678".into()])
                    .with_roster(vec!["abc".into(), "def".into()])
                    .with_persistence(PersistenceType::Any)
                    .with_durability(DurabilityType::NoDurability)
                    .with_routes(RouteType::Any)
                    .with_circuit_management_type("admin_test_app".into())
                    .build()
                    .expect("Should have built a correct circuit");

                let mut circuit_directory = CircuitDirectory::new();
                circuit_directory.add_circuit("alpha".to_string(), circuit);

                let state = SplinterState::new("memory".to_string(), circuit_directory);

                let handler = AdminDirectMessageHandler::new("1234".into(), state);
                dispatcher.set_handler(Box::new(handler));

                let mut direct_message = AdminDirectMessage::new();
                direct_message.set_circuit("admin".into());
                direct_message.set_sender("admin::5678".into());
                direct_message.set_recipient("def".into());
                direct_message.set_payload(b"test".to_vec());
                direct_message.set_correlation_id("random_corr_id".into());
                let direct_bytes = direct_message.write_to_bytes().unwrap();

                assert_eq!(
                    Ok(()),
                    dispatcher.dispatch(
                        "5678".into(),
                        &CircuitMessageType::ADMIN_DIRECT_MESSAGE,
                        direct_bytes
                    )
                );
            },
            "1234",
            CircuitMessageType::CIRCUIT_ERROR_MESSAGE,
            |error_msg: CircuitError| {
                assert_eq!(error_msg.get_service_id(), "admin::5678");
                assert_eq!(
                    error_msg.get_error(),
                    CircuitError_Error::ERROR_RECIPIENT_NOT_IN_CIRCUIT_ROSTER,
                );
                assert_eq!(error_msg.get_correlation_id(), "random_corr_id");
            },
        )
    }

    /// Send a message to an admin service via the standard circuit. Expect that the message is
    /// sent to the current node's target admin service.
    #[test]
    fn test_send_admin_direct_message_via_standard_circuit() {
        run_test(
            |mut listener, mut dispatcher, network1| {
                let connection = listener.accept().expect("Cannot accept connection");
                network1
                    .add_peer("5678".to_string(), connection)
                    .expect("Unable to add peer");
                // Add circuit and service to splinter state
                let circuit = Circuit::builder()
                    .with_id("alpha".into())
                    .with_auth(AuthorizationType::Trust)
                    .with_members(vec!["1234".into(), "5678".into()])
                    .with_roster(vec!["abc".into(), "def".into()])
                    .with_persistence(PersistenceType::Any)
                    .with_durability(DurabilityType::NoDurability)
                    .with_routes(RouteType::Any)
                    .with_circuit_management_type("admin_test_app".into())
                    .build()
                    .expect("Should have built a correct circuit");

                let mut circuit_directory = CircuitDirectory::new();
                circuit_directory.add_circuit("alpha".to_string(), circuit);

                let state = SplinterState::new("memory".to_string(), circuit_directory);

                let handler = AdminDirectMessageHandler::new("1234".into(), state);
                dispatcher.set_handler(Box::new(handler));

                let mut direct_message = AdminDirectMessage::new();
                direct_message.set_circuit("alpha".into());
                direct_message.set_sender("admin::1234".into());
                direct_message.set_recipient("admin::5678".into());
                direct_message.set_payload(b"test".to_vec());
                direct_message.set_correlation_id("random_corr_id".into());
                let direct_bytes = direct_message.write_to_bytes().unwrap();

                assert_eq!(
                    Ok(()),
                    dispatcher.dispatch(
                        "1234".into(),
                        &CircuitMessageType::ADMIN_DIRECT_MESSAGE,
                        direct_bytes
                    )
                );
            },
            "1234",
            CircuitMessageType::ADMIN_DIRECT_MESSAGE,
            |msg: AdminDirectMessage| {
                assert_eq!(msg.get_circuit(), "alpha");
                assert_eq!(msg.get_sender(), "admin::1234");
                assert_eq!(msg.get_recipient(), "admin::5678");
                assert_eq!(msg.get_payload(), b"test");
                assert_eq!(msg.get_correlation_id(), "random_corr_id");
            },
        )
    }

    /// Send a message to an admin service via the admin circuit. Expect that the message is
    /// sent to the appropriate node that hosts the target admin service.
    #[test]
    fn test_send_admin_direct_message_via_admin_circuit() {
        run_test(
            |mut listener, mut dispatcher, network1| {
                let connection = listener.accept().expect("Cannot accept connection");
                network1
                    .add_peer("5678".to_string(), connection)
                    .expect("Unable to add peer");
                let circuit_directory = CircuitDirectory::new();

                let state = SplinterState::new("memory".to_string(), circuit_directory);

                let handler = AdminDirectMessageHandler::new("1234".into(), state);
                dispatcher.set_handler(Box::new(handler));

                let mut direct_message = AdminDirectMessage::new();
                direct_message.set_circuit("admin".into());
                direct_message.set_sender("admin::1234".into());
                direct_message.set_recipient("admin::5678".into());
                direct_message.set_payload(b"test".to_vec());
                direct_message.set_correlation_id("random_corr_id".into());
                let direct_bytes = direct_message.write_to_bytes().unwrap();

                assert_eq!(
                    Ok(()),
                    dispatcher.dispatch(
                        "1234".into(),
                        &CircuitMessageType::ADMIN_DIRECT_MESSAGE,
                        direct_bytes
                    )
                );
            },
            "1234",
            CircuitMessageType::ADMIN_DIRECT_MESSAGE,
            |msg: AdminDirectMessage| {
                assert_eq!(msg.get_circuit(), "admin");
                assert_eq!(msg.get_sender(), "admin::1234");
                assert_eq!(msg.get_recipient(), "admin::5678");
                assert_eq!(msg.get_payload(), b"test");
                assert_eq!(msg.get_correlation_id(), "random_corr_id");
            },
        )
    }

    /// Send a message to an admin service via the standard circuit.  Expect that the message is
    /// sent to the current node's target admin service.
    #[test]
    fn test_send_admin_direct_message_via_admin_circuit_to_local_service() {
        run_test(
            |mut listener, mut dispatcher, network1| {
                let connection = listener.accept().expect("Cannot accept connection");
                network1
                    .add_peer("admin::1234".to_string(), connection)
                    .expect("Unable to add peer");
                let circuit_directory = CircuitDirectory::new();

                let state = SplinterState::new("memory".to_string(), circuit_directory);

                let handler = AdminDirectMessageHandler::new("1234".into(), state);
                dispatcher.set_handler(Box::new(handler));

                let mut direct_message = AdminDirectMessage::new();
                direct_message.set_circuit("admin".into());
                direct_message.set_sender("admin::5678".into());
                direct_message.set_recipient("admin::1234".into());
                direct_message.set_payload(b"test".to_vec());
                direct_message.set_correlation_id("random_corr_id".into());
                let direct_bytes = direct_message.write_to_bytes().unwrap();

                assert_eq!(
                    Ok(()),
                    dispatcher.dispatch(
                        "1234".into(),
                        &CircuitMessageType::ADMIN_DIRECT_MESSAGE,
                        direct_bytes
                    )
                );
            },
            "1234",
            CircuitMessageType::ADMIN_DIRECT_MESSAGE,
            |msg: AdminDirectMessage| {
                assert_eq!(msg.get_circuit(), "admin");
                assert_eq!(msg.get_sender(), "admin::5678");
                assert_eq!(msg.get_recipient(), "admin::1234");
                assert_eq!(msg.get_payload(), b"test");
                assert_eq!(msg.get_correlation_id(), "random_corr_id");
            },
        )
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
        let dispatcher = Dispatcher::new(Box::new(network_sender));
        let listener = inproc_transport
            .listen("inproc://admin_message")
            .expect("Cannot get listener");

        std::thread::spawn(move || test(listener, dispatcher, network1));

        let mesh2 = Mesh::new(1, 1);
        let network2 = Network::new(mesh2.clone(), 0).unwrap();
        let connection = inproc_transport
            .connect("inproc://admin_message")
            .expect("Unable to connect to inproc");
        network2
            .add_peer("1234".to_string(), connection)
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
