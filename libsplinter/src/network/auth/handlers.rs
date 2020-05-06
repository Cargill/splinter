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

use protobuf::Message;

use crate::network::auth::{
    AuthorizationAction, AuthorizationInquisitor, AuthorizationManager, AuthorizationState,
};
use crate::network::dispatch::{
    DispatchError, DispatchMessageSender, Dispatcher, FromMessageBytes, Handler, MessageContext,
    MessageSender, PeerId,
};
use crate::network::sender::NetworkMessageSender;
use crate::protos::authorization::{
    AuthorizationError, AuthorizationMessage, AuthorizationMessageType, AuthorizedMessage,
    ConnectRequest, ConnectRequest_HandshakeMode, ConnectResponse,
    ConnectResponse_AuthorizationType, TrustRequest,
};
use crate::protos::network::{NetworkMessage, NetworkMessageType};

/// Create a Dispatcher for Authorization messages
///
/// Creates and configures a Dispatcher to handle messages from an AuthorizationMessage envelope.
/// The dispatcher is provided the given network sender for response messages, and the network
/// itself to handle updating identities (or removing connections with authorization failures).
///
/// The identity provided is sent to connections for Trust authorizations.
pub fn create_authorization_dispatcher(
    auth_manager: AuthorizationManager,
    network_sender: NetworkMessageSender,
) -> Dispatcher<AuthorizationMessageType> {
    let mut auth_dispatcher = Dispatcher::new(Box::new(network_sender));

    auth_dispatcher.set_handler(Box::new(ConnectRequestHandler::new(auth_manager.clone())));

    auth_dispatcher.set_handler(Box::new(ConnectResponseHandler::new(auth_manager.clone())));

    auth_dispatcher.set_handler(Box::new(TrustRequestHandler::new(auth_manager.clone())));

    auth_dispatcher.set_handler(Box::new(AuthorizedHandler));

    auth_dispatcher.set_handler(Box::new(AuthorizationErrorHandler::new(auth_manager)));

    auth_dispatcher
}

/// The Handler for authorization network messages.
///
/// This Handler accepts authorization network messages, unwraps the envelope, and forwards the
/// message contents to an authorization dispatcher.
pub struct AuthorizationMessageHandler {
    sender: DispatchMessageSender<AuthorizationMessageType>,
}

impl AuthorizationMessageHandler {
    /// Constructs a new AuthorizationMessageHandler
    ///
    /// This constructs an AuthorizationMessageHandler with a sender that will dispatch messages
    /// to a authorization dispatcher.
    pub fn new(sender: DispatchMessageSender<AuthorizationMessageType>) -> Self {
        AuthorizationMessageHandler { sender }
    }
}

impl Handler for AuthorizationMessageHandler {
    type Source = PeerId;
    type MessageType = NetworkMessageType;
    type Message = AuthorizationMessage;

    fn match_type(&self) -> Self::MessageType {
        NetworkMessageType::AUTHORIZATION
    }

    fn handle(
        &self,
        mut msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        _sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        self.sender
            .send(
                msg.message_type,
                msg.take_payload(),
                context.source_id().clone(),
            )
            .map_err(|(_, message_bytes, source_id)| {
                DispatchError::NetworkSendError((source_id.into(), message_bytes))
            })?;
        Ok(())
    }
}

pub struct AuthorizedHandler;

impl Handler for AuthorizedHandler {
    type Source = PeerId;
    type MessageType = AuthorizationMessageType;
    type Message = AuthorizedMessage;

    fn match_type(&self) -> Self::MessageType {
        AuthorizationMessageType::AUTHORIZE
    }

    fn handle(
        &self,
        _: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        _sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        info!(
            "Connection authorized with peer {}",
            context.source_peer_id()
        );
        Ok(())
    }
}

/// Guards handlers to ensure that they are authorized, before allowing the wrapped handler to be
/// called.
///
/// Specifically, this guards messages at the network level, so the handler is fixed to the
/// NetworkMessageType.
pub struct NetworkAuthGuardHandler<M: FromMessageBytes> {
    auth_manager: AuthorizationManager,
    handler: Box<dyn Handler<Source = PeerId, MessageType = NetworkMessageType, Message = M>>,
}

impl<M: FromMessageBytes> NetworkAuthGuardHandler<M> {
    /// Constructs a new handler.
    ///
    /// Handlers must be typed to the NetworkMessageType, but may be any message content type.
    pub fn new(
        auth_manager: AuthorizationManager,
        handler: Box<dyn Handler<Source = PeerId, MessageType = NetworkMessageType, Message = M>>,
    ) -> Self {
        NetworkAuthGuardHandler {
            auth_manager,
            handler,
        }
    }
}

impl<M: FromMessageBytes> Handler for NetworkAuthGuardHandler<M> {
    type Source = PeerId;
    type MessageType = NetworkMessageType;
    type Message = M;

    fn match_type(&self) -> Self::MessageType {
        self.handler.match_type()
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        if self.auth_manager.is_authorized(context.source_peer_id()) {
            self.handler.handle(msg, context, sender)
        } else {
            debug!(
                "{} attempting to send {:?} message before completing authorization",
                context.source_peer_id(),
                context.message_type()
            );
            Ok(())
        }
    }
}

/// Handler for the Connect Request Authorization Message Type
struct ConnectRequestHandler {
    auth_manager: AuthorizationManager,
}

impl ConnectRequestHandler {
    fn new(auth_manager: AuthorizationManager) -> Self {
        ConnectRequestHandler { auth_manager }
    }
}

impl Handler for ConnectRequestHandler {
    type Source = PeerId;
    type MessageType = AuthorizationMessageType;
    type Message = ConnectRequest;

    fn match_type(&self) -> Self::MessageType {
        AuthorizationMessageType::CONNECT_REQUEST
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        match self
            .auth_manager
            .next_state(context.source_peer_id(), AuthorizationAction::Connecting)
        {
            Err(err) => {
                debug!(
                    "Ignoring duplicate connect message from peer {}: {}",
                    context.source_peer_id(),
                    err
                );
            }
            Ok(AuthorizationState::Connecting) => {
                debug!("Beginning handshake for peer {}", context.source_peer_id(),);
                // Send a connect request of our own

                if msg.get_handshake_mode() == ConnectRequest_HandshakeMode::BIDIRECTIONAL {
                    let mut connect_req = ConnectRequest::new();
                    connect_req.set_handshake_mode(ConnectRequest_HandshakeMode::UNIDIRECTIONAL);
                    sender
                        .send(
                            context.source_id().clone(),
                            wrap_in_network_auth_envelopes(
                                AuthorizationMessageType::CONNECT_REQUEST,
                                connect_req,
                            )?,
                        )
                        .map_err(|(recipient, payload)| {
                            DispatchError::NetworkSendError((recipient.into(), payload))
                        })?;

                    debug!(
                        "Sent bidirectional connect request to peer {}",
                        context.source_peer_id()
                    );
                }

                let mut response = ConnectResponse::new();
                response.set_accepted_authorization_types(vec![
                    ConnectResponse_AuthorizationType::TRUST,
                ]);
                sender
                    .send(
                        context.source_id().clone(),
                        wrap_in_network_auth_envelopes(
                            AuthorizationMessageType::CONNECT_RESPONSE,
                            response,
                        )?,
                    )
                    .map_err(|(recipient, payload)| {
                        DispatchError::NetworkSendError((recipient.into(), payload))
                    })?;
            }
            Ok(AuthorizationState::Internal) => {
                debug!(
                    "Sending Authorized message to internal peer {}",
                    context.source_peer_id()
                );
                let auth_msg = AuthorizedMessage::new();
                sender
                    .send(
                        context.source_id().clone(),
                        wrap_in_network_auth_envelopes(
                            AuthorizationMessageType::AUTHORIZE,
                            auth_msg,
                        )?,
                    )
                    .map_err(|(recipient, payload)| {
                        DispatchError::NetworkSendError((recipient.into(), payload))
                    })?;
            }
            Ok(next_state) => panic!("Should not have been able to transition to {}", next_state),
        }

        Ok(())
    }
}

/// Handler for the ConnectResponse Authorization Message Type
struct ConnectResponseHandler {
    auth_manager: AuthorizationManager,
}

impl ConnectResponseHandler {
    fn new(auth_manager: AuthorizationManager) -> Self {
        ConnectResponseHandler { auth_manager }
    }
}

impl Handler for ConnectResponseHandler {
    type Source = PeerId;
    type MessageType = AuthorizationMessageType;
    type Message = ConnectResponse;

    fn match_type(&self) -> Self::MessageType {
        AuthorizationMessageType::CONNECT_RESPONSE
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        debug!(
            "Receive connect response from peer {}: {:?}",
            context.source_peer_id(),
            msg
        );
        if msg
            .get_accepted_authorization_types()
            .iter()
            .any(|t| t == &ConnectResponse_AuthorizationType::TRUST)
        {
            let mut trust_request = TrustRequest::new();
            trust_request.set_identity(self.auth_manager.identity.clone());
            sender
                .send(
                    context.source_id().clone(),
                    wrap_in_network_auth_envelopes(
                        AuthorizationMessageType::TRUST_REQUEST,
                        trust_request,
                    )?,
                )
                .map_err(|(recipient, payload)| {
                    DispatchError::NetworkSendError((recipient.into(), payload))
                })?;
        }
        Ok(())
    }
}

/// Handler for the TrustRequest Authorization Message Type
struct TrustRequestHandler {
    auth_manager: AuthorizationManager,
}

impl TrustRequestHandler {
    fn new(auth_manager: AuthorizationManager) -> Self {
        TrustRequestHandler { auth_manager }
    }
}

impl Handler for TrustRequestHandler {
    type Source = PeerId;
    type MessageType = AuthorizationMessageType;
    type Message = TrustRequest;

    fn match_type(&self) -> Self::MessageType {
        AuthorizationMessageType::TRUST_REQUEST
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        match self.auth_manager.next_state(
            context.source_peer_id(),
            AuthorizationAction::TrustIdentifying(msg.get_identity().to_string()),
        ) {
            Err(err) => {
                debug!(
                    "Ignoring trust request message from peer {}: {}",
                    context.source_peer_id(),
                    err
                );
            }
            Ok(AuthorizationState::Authorized) => {
                debug!(
                    "Sending Authorized message to peer {} (formerly {})",
                    msg.get_identity(),
                    context.source_peer_id()
                );
                let auth_msg = AuthorizedMessage::new();
                sender
                    .send(
                        msg.get_identity().into(),
                        wrap_in_network_auth_envelopes(
                            AuthorizationMessageType::AUTHORIZE,
                            auth_msg,
                        )?,
                    )
                    .map_err(|(recipient, payload)| {
                        DispatchError::NetworkSendError((recipient.into(), payload))
                    })?;
            }
            Ok(next_state) => panic!("Should not have been able to transition to {}", next_state),
        }
        Ok(())
    }
}

/// Handler for the Authorization Error Message Type
struct AuthorizationErrorHandler {
    auth_manager: AuthorizationManager,
}

impl AuthorizationErrorHandler {
    fn new(auth_manager: AuthorizationManager) -> Self {
        AuthorizationErrorHandler { auth_manager }
    }
}

impl Handler for AuthorizationErrorHandler {
    type Source = PeerId;
    type MessageType = AuthorizationMessageType;
    type Message = AuthorizationError;

    fn match_type(&self) -> Self::MessageType {
        AuthorizationMessageType::AUTHORIZATION_ERROR
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        _: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        match self
            .auth_manager
            .next_state(context.source_peer_id(), AuthorizationAction::Unauthorizing)
        {
            Ok(AuthorizationState::Unauthorized) => {
                info!(
                    "Connection unauthorized by peer {}: {}",
                    context.source_peer_id(),
                    msg.get_error_message()
                );
            }
            Err(err) => {
                warn!(
                    "Unable to handle unauthorizing by peer {}: {}",
                    context.source_peer_id(),
                    err
                );
            }
            Ok(next_state) => panic!("Should not have been able to transition to {}", next_state),
        }
        Ok(())
    }
}

fn wrap_in_network_auth_envelopes<M: protobuf::Message>(
    msg_type: AuthorizationMessageType,
    auth_msg: M,
) -> Result<Vec<u8>, DispatchError> {
    let mut auth_msg_env = AuthorizationMessage::new();
    auth_msg_env.set_message_type(msg_type);
    auth_msg_env.set_payload(auth_msg.write_to_bytes()?);

    let mut network_msg = NetworkMessage::new();
    network_msg.set_message_type(NetworkMessageType::AUTHORIZATION);
    network_msg.set_payload(auth_msg_env.write_to_bytes()?);

    network_msg.write_to_bytes().map_err(DispatchError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::Duration;

    use protobuf::Message;

    use crate::mesh::Mesh;
    use crate::network::sender;
    use crate::network::Network;
    use crate::protos::authorization::{
        AuthorizationError, AuthorizationError_AuthorizationErrorType, AuthorizationMessage,
        AuthorizedMessage, ConnectRequest, ConnectResponse, ConnectResponse_AuthorizationType,
        TrustRequest,
    };
    use crate::protos::network::{NetworkMessage, NetworkMessageType};
    use crate::transport::socket::TcpTransport;
    use crate::transport::{
        ConnectError, Connection, DisconnectError, RecvError, SendError, Transport,
    };

    #[test]
    fn connect_request_dispatch() {
        let (network1, peer_id) = create_network_with_initial_temp_peer();

        let auth_mgr = AuthorizationManager::new(network1.clone(), "mock_identity".into());
        let network_message_queue = sender::Builder::new()
            .with_network(network1.clone())
            .build()
            .expect("Unable to create queue");
        let network_sender = network_message_queue.new_network_sender();

        let mut tcp_transport = TcpTransport::default();
        let mut listener = tcp_transport
            .listen("tcp://localhost:0")
            .expect("Cannot listen for connections");
        let endpoint = listener.endpoint();

        let dispatcher = create_authorization_dispatcher(auth_mgr, network_sender);

        std::thread::spawn(move || {
            let connection = listener.accept().expect("Cannot accept connection");
            network1
                .add_peer(peer_id.clone(), connection)
                .expect("Unable to add peer");

            let mut msg = ConnectRequest::new();
            msg.set_handshake_mode(ConnectRequest_HandshakeMode::BIDIRECTIONAL);
            let msg_bytes = msg.write_to_bytes().expect("Unable to serialize message");
            assert_eq!(
                Ok(()),
                dispatcher.dispatch(
                    peer_id.into(),
                    &AuthorizationMessageType::CONNECT_REQUEST,
                    msg_bytes
                )
            );
        });

        let mesh2 = Mesh::new(1, 1);
        let network2 = Network::new(mesh2.clone(), 0).unwrap();
        let connection = tcp_transport
            .connect(&endpoint)
            .expect("Unable to connect to inproc");
        network2
            .add_peer("mock_identity".to_string(), connection)
            .expect("Unable to add peer");

        let network_message = network2
            .recv()
            .expect("Unable to receive message over the network");
        let connect_req_msg: ConnectRequest = expect_auth_message(
            AuthorizationMessageType::CONNECT_REQUEST,
            network_message.payload(),
        );
        assert_eq!(
            ConnectRequest_HandshakeMode::UNIDIRECTIONAL,
            connect_req_msg.get_handshake_mode()
        );

        let network_message = network2
            .recv()
            .expect("Unable to receive message over the network");

        let connect_res_msg: ConnectResponse = expect_auth_message(
            AuthorizationMessageType::CONNECT_RESPONSE,
            network_message.payload(),
        );
        assert_eq!(
            vec![ConnectResponse_AuthorizationType::TRUST],
            connect_res_msg.get_accepted_authorization_types().to_vec()
        );
    }

    // Test that a connect response is properly dispatched
    // There should be a trust request sent to the responding peer
    #[test]
    fn connect_response_dispatch() {
        let (network, peer_id) = create_network_with_initial_temp_peer();
        let auth_mgr = AuthorizationManager::new(network.clone(), "mock_identity".into());

        let network_message_queue = sender::Builder::new()
            .with_network(network.clone())
            .build()
            .expect("Unable to create queue");
        let network_sender = network_message_queue.new_network_sender();

        let mut tcp_transport = TcpTransport::default();
        let mut listener = tcp_transport
            .listen("tcp://localhost:0")
            .expect("Cannot listen for connections");
        let endpoint = listener.endpoint();
        let dispatcher = create_authorization_dispatcher(auth_mgr, network_sender);

        std::thread::spawn(move || {
            let connection = listener.accept().expect("Cannot accept connection");
            network
                .add_peer(peer_id.clone(), connection)
                .expect("Unable to add peer");
            let mut msg = ConnectResponse::new();
            msg.set_accepted_authorization_types(
                vec![ConnectResponse_AuthorizationType::TRUST].into(),
            );
            let msg_bytes = msg.write_to_bytes().expect("Unable to serialize message");
            assert_eq!(
                Ok(()),
                dispatcher.dispatch(
                    peer_id.into(),
                    &AuthorizationMessageType::CONNECT_RESPONSE,
                    msg_bytes
                )
            );
        });

        let mesh2 = Mesh::new(1, 1);
        let network2 = Network::new(mesh2.clone(), 0).unwrap();
        let connection = tcp_transport
            .connect(&endpoint)
            .expect("Unable to connect to inproc");
        network2
            .add_peer("mock_identity".to_string(), connection)
            .expect("Unable to add peer");

        let network_message = network2
            .recv()
            .expect("Unable to receive message over the network");

        let trust_req: TrustRequest = expect_auth_message(
            AuthorizationMessageType::TRUST_REQUEST,
            network_message.payload(),
        );
        assert_eq!("mock_identity", trust_req.get_identity());
    }

    // Test that the node can handle a trust response
    #[test]
    fn trust_request_dispatch() {
        let (network, peer_id) = create_network_with_initial_temp_peer();

        let auth_mgr = AuthorizationManager::new(network.clone(), "mock_identity".into());
        let network_message_queue = sender::Builder::new()
            .with_network(network.clone())
            .build()
            .expect("Unable to create queue");
        let network_sender = network_message_queue.new_network_sender();

        let mut tcp_transport = TcpTransport::default();
        let mut listener = tcp_transport
            .listen("tcp://localhost:0")
            .expect("Cannot listen for connections");
        let endpoint = listener.endpoint();
        let dispatcher = create_authorization_dispatcher(auth_mgr, network_sender);

        std::thread::spawn(move || {
            let connection = listener.accept().expect("Cannot accept connection");
            network
                .add_peer(peer_id.clone(), connection)
                .expect("Unable to add peer");
            // Begin the connection process, otherwise, the response will fail
            let mut msg = ConnectRequest::new();
            msg.set_handshake_mode(ConnectRequest_HandshakeMode::UNIDIRECTIONAL);
            let msg_bytes = msg.write_to_bytes().expect("Unable to serialize message");
            assert_eq!(
                Ok(()),
                dispatcher.dispatch(
                    peer_id.clone().into(),
                    &AuthorizationMessageType::CONNECT_REQUEST,
                    msg_bytes
                )
            );

            let mut trust_req = TrustRequest::new();
            trust_req.set_identity("my_identity".into());
            let msg_bytes = trust_req
                .write_to_bytes()
                .expect("Unable to serialize message");
            assert_eq!(
                Ok(()),
                dispatcher.dispatch(
                    peer_id.into(),
                    &AuthorizationMessageType::TRUST_REQUEST,
                    msg_bytes
                )
            );
        });

        let mesh2 = Mesh::new(1, 1);
        let network2 = Network::new(mesh2.clone(), 0).unwrap();
        let connection = tcp_transport
            .connect(&endpoint)
            .expect("Unable to connect to inproc");
        network2
            .add_peer("mock_identity".to_string(), connection)
            .expect("Unable to add peer");

        let network_message = network2
            .recv()
            .expect("Unable to receive message over the network");

        let _connect_res_msg: ConnectResponse = expect_auth_message(
            AuthorizationMessageType::CONNECT_RESPONSE,
            network_message.payload(),
        );

        let network_message = network2
            .recv()
            .expect("Unable to receive message over the network");

        let _auth_msg: AuthorizedMessage = expect_auth_message(
            AuthorizationMessageType::AUTHORIZE,
            network_message.payload(),
        );
    }

    // Test that an AuthorizationError message is properly handled
    // 1. Configure the dispatcher
    // 2. Dispatch a connect message for a peer id
    // 3. Dispatch the error message for the same peer id
    // 4. Verify that the connection is dropped from the network.
    #[test]
    fn auth_error_dispatch() {
        let (network, peer_id) = create_network_with_initial_temp_peer();

        let auth_mgr = AuthorizationManager::new(network.clone(), "mock_pub_key".into());
        let network_message_queue = sender::Builder::new()
            .with_network(network.clone())
            .build()
            .expect("Unable to create queue");
        let network_sender = network_message_queue.new_network_sender();

        let mut tcp_transport = TcpTransport::default();
        let mut listener = tcp_transport
            .listen("tcp://localhost:0")
            .expect("Cannot listen for connections");
        let endpoint = listener.endpoint();
        let dispatcher = create_authorization_dispatcher(auth_mgr, network_sender);

        std::thread::spawn(move || {
            let connection = listener.accept().expect("Cannot accept connection");
            network
                .add_peer(peer_id.clone(), connection)
                .expect("Unable to add peer");
            // Begin the connection process, otherwise, the response will fail
            let mut msg = ConnectRequest::new();
            msg.set_handshake_mode(ConnectRequest_HandshakeMode::UNIDIRECTIONAL);
            let msg_bytes = msg.write_to_bytes().expect("Unable to serialize message");
            assert_eq!(
                Ok(()),
                dispatcher.dispatch(
                    peer_id.clone().into(),
                    &AuthorizationMessageType::CONNECT_REQUEST,
                    msg_bytes
                )
            );
            let _network_msg = network.recv();

            let mut error_message = AuthorizationError::new();
            error_message
                .set_error_type(AuthorizationError_AuthorizationErrorType::AUTHORIZATION_REJECTED);
            error_message.set_error_message("Test Error!".into());
            let msg_bytes = error_message
                .write_to_bytes()
                .expect("Unable to serialize error message");

            assert_eq!(
                Ok(()),
                dispatcher.dispatch(
                    peer_id.into(),
                    &AuthorizationMessageType::AUTHORIZATION_ERROR,
                    msg_bytes
                )
            );

            assert_eq!(0, network.peer_ids().len());
        });

        let mesh2 = Mesh::new(1, 1);
        let network2 = Network::new(mesh2.clone(), 0).unwrap();
        let connection = tcp_transport
            .connect(&endpoint)
            .expect("Unable to connect to inproc");
        network2
            .add_peer("mock_identity".to_string(), connection)
            .expect("Unable to add peer");

        let network_message = network2
            .recv()
            .expect("Unable to receive message over the network");

        let _connect_res_msg: ConnectResponse = expect_auth_message(
            AuthorizationMessageType::CONNECT_RESPONSE,
            network_message.payload(),
        );

        let timeout = Duration::from_secs(1);
        if let Ok(_) = network2.recv_timeout(timeout) {
            panic!("No messsages should have been sent")
        }
    }

    fn expect_auth_message<M: protobuf::Message>(
        message_type: AuthorizationMessageType,
        msg_bytes: &[u8],
    ) -> M {
        let network_msg: NetworkMessage =
            protobuf::parse_from_bytes(msg_bytes).expect("Unable to parse network message");
        assert_eq!(NetworkMessageType::AUTHORIZATION, network_msg.message_type);

        let auth_msg: AuthorizationMessage = protobuf::parse_from_bytes(network_msg.get_payload())
            .expect("Unable to parse auth message");

        assert_eq!(message_type, auth_msg.message_type);

        match protobuf::parse_from_bytes(auth_msg.get_payload()) {
            Ok(msg) => msg,
            Err(err) => panic!(
                "unable to parse message for type {:?}: {:?}",
                message_type, err
            ),
        }
    }

    fn create_network_with_initial_temp_peer() -> (Network, String) {
        let network = Network::new(Mesh::new(5, 5), 0).unwrap();

        let mut transport = MockConnectingTransport;

        let connection = transport
            .connect("local")
            .expect("Unable to create the connection");

        network
            .add_connection(connection)
            .expect("Unable to add connection to network");

        // We only have one peer, so we can grab this id as the temp id.
        let peer_id = network.peer_ids()[0].clone();

        (network, peer_id)
    }

    struct MockConnectingTransport;

    impl Transport for MockConnectingTransport {
        fn accepts(&self, _: &str) -> bool {
            true
        }

        fn connect(&mut self, _: &str) -> Result<Box<dyn Connection>, ConnectError> {
            Ok(Box::new(MockConnection))
        }

        fn listen(
            &mut self,
            _: &str,
        ) -> Result<Box<dyn crate::transport::Listener>, crate::transport::ListenError> {
            unimplemented!()
        }
    }

    struct MockConnection;

    impl Connection for MockConnection {
        fn send(&mut self, _message: &[u8]) -> Result<(), SendError> {
            Ok(())
        }

        fn recv(&mut self) -> Result<Vec<u8>, RecvError> {
            unimplemented!()
        }

        fn remote_endpoint(&self) -> String {
            String::from("MockConnection")
        }

        fn local_endpoint(&self) -> String {
            String::from("MockConnection")
        }

        fn disconnect(&mut self) -> Result<(), DisconnectError> {
            Ok(())
        }

        fn evented(&self) -> &dyn mio::Evented {
            &MockEvented
        }
    }

    struct MockEvented;

    impl mio::Evented for MockEvented {
        fn register(
            &self,
            _poll: &mio::Poll,
            _token: mio::Token,
            _interest: mio::Ready,
            _opts: mio::PollOpt,
        ) -> std::io::Result<()> {
            Ok(())
        }

        fn reregister(
            &self,
            _poll: &mio::Poll,
            _token: mio::Token,
            _interest: mio::Ready,
            _opts: mio::PollOpt,
        ) -> std::io::Result<()> {
            Ok(())
        }

        fn deregister(&self, _poll: &mio::Poll) -> std::io::Result<()> {
            Ok(())
        }
    }
}
