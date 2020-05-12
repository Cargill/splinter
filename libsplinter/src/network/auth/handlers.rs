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

use crate::network::dispatch::{
    ConnectionId, DispatchError, Dispatcher, Handler, MessageContext, MessageSender,
};
use crate::protocol::authorization::{
    AuthorizationError, AuthorizationMessage, AuthorizationType, Authorized, ConnectRequest,
    ConnectResponse, TrustRequest,
};
use crate::protos::authorization;
use crate::protos::network::{NetworkMessage, NetworkMessageType};
use crate::protos::prelude::*;

use super::{
    AuthorizationAction, AuthorizationManagerStateMachine, AuthorizationMessageSender,
    AuthorizationState,
};

/// Create a Dispatcher for Authorization messages
///
/// Creates and configures a Dispatcher to handle messages from an AuthorizationMessage envelope.
/// The dispatcher is provided the given network sender for response messages, and the network
/// itself to handle updating identities (or removing connections with authorization failures).
///
/// The identity provided is sent to connections for Trust authorizations.
pub fn create_authorization_dispatcher(
    identity: String,
    auth_manager: AuthorizationManagerStateMachine,
    auth_msg_sender: impl MessageSender<ConnectionId> + Clone + 'static,
) -> Dispatcher<NetworkMessageType, ConnectionId> {
    let mut auth_dispatcher = Dispatcher::new(Box::new(auth_msg_sender.clone()));

    auth_dispatcher.set_handler(Box::new(ConnectRequestHandler::new(auth_manager.clone())));

    auth_dispatcher.set_handler(Box::new(ConnectResponseHandler::new(identity)));

    auth_dispatcher.set_handler(Box::new(TrustRequestHandler::new(auth_manager.clone())));

    auth_dispatcher.set_handler(Box::new(AuthorizedHandler));

    auth_dispatcher.set_handler(Box::new(AuthorizationErrorHandler::new(auth_manager)));

    let mut network_msg_dispatcher = Dispatcher::new(Box::new(auth_msg_sender));

    network_msg_dispatcher.set_handler(Box::new(AuthorizationMessageHandler::new(auth_dispatcher)));

    network_msg_dispatcher
}

/// The Handler for authorization network messages.
///
/// This Handler accepts authorization network messages, unwraps the envelope, and forwards the
/// message contents to an authorization dispatcher.
pub struct AuthorizationMessageHandler {
    auth_dispatcher: Dispatcher<authorization::AuthorizationMessageType, ConnectionId>,
}

impl AuthorizationMessageHandler {
    /// Constructs a new AuthorizationMessageHandler
    ///
    /// This constructs an AuthorizationMessageHandler with a sender that will dispatch messages
    /// to a authorization dispatcher.
    pub fn new(
        auth_dispatcher: Dispatcher<authorization::AuthorizationMessageType, ConnectionId>,
    ) -> Self {
        AuthorizationMessageHandler { auth_dispatcher }
    }
}

impl Handler for AuthorizationMessageHandler {
    type Source = ConnectionId;
    type MessageType = NetworkMessageType;
    type Message = authorization::AuthorizationMessage;

    fn match_type(&self) -> Self::MessageType {
        NetworkMessageType::AUTHORIZATION
    }

    fn handle(
        &self,
        mut msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        _sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        let msg_type = msg.get_message_type();
        let payload = msg.take_payload();
        self.auth_dispatcher
            .dispatch(context.source_id().clone(), &msg_type, payload)
    }
}

pub struct AuthorizedHandler;

impl Handler for AuthorizedHandler {
    type Source = ConnectionId;
    type MessageType = authorization::AuthorizationMessageType;
    type Message = authorization::AuthorizedMessage;

    fn match_type(&self) -> Self::MessageType {
        authorization::AuthorizationMessageType::AUTHORIZE
    }

    fn handle(
        &self,
        _: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        _sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        debug!("Connection {} authorized", context.source_connection_id());
        Ok(())
    }
}
///
/// Handler for the Connect Request Authorization Message Type
struct ConnectRequestHandler {
    auth_manager: AuthorizationManagerStateMachine,
}

impl ConnectRequestHandler {
    fn new(auth_manager: AuthorizationManagerStateMachine) -> Self {
        ConnectRequestHandler { auth_manager }
    }
}

impl Handler for ConnectRequestHandler {
    type Source = ConnectionId;
    type MessageType = authorization::AuthorizationMessageType;
    type Message = authorization::ConnectRequest;

    fn match_type(&self) -> Self::MessageType {
        authorization::AuthorizationMessageType::CONNECT_REQUEST
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        let connect_request = ConnectRequest::from_proto(msg)?;
        match self.auth_manager.next_state(
            context.source_connection_id(),
            AuthorizationAction::Connecting,
        ) {
            Err(err) => {
                debug!(
                    "Ignoring duplicate connect message from {}: {}",
                    context.source_connection_id(),
                    err
                );
            }
            Ok(AuthorizationState::Connecting) => {
                debug!("Beginning handshake for {}", context.source_connection_id(),);
                // Send a connect request of our own

                match connect_request {
                    ConnectRequest::Bidirectional => {
                        let connect_req =
                            AuthorizationMessage::ConnectRequest(ConnectRequest::Unidirectional);
                        let mut msg = NetworkMessage::new();
                        msg.set_message_type(NetworkMessageType::AUTHORIZATION);
                        msg.set_payload(
                            IntoBytes::<authorization::AuthorizationMessage>::into_bytes(
                                connect_req,
                            )?,
                        );
                        sender
                            .send(context.source_id().clone(), msg.write_to_bytes()?)
                            .map_err(|(recipient, payload)| {
                                DispatchError::NetworkSendError((recipient.into(), payload))
                            })?;

                        debug!(
                            "Sent bidirectional connect request to {}",
                            context.source_connection_id()
                        );
                    }
                    ConnectRequest::Unidirectional => (),
                }

                let response = AuthorizationMessage::ConnectResponse(ConnectResponse {
                    accepted_authorization_types: vec![AuthorizationType::Trust],
                });

                let mut msg = NetworkMessage::new();
                msg.set_message_type(NetworkMessageType::AUTHORIZATION);
                msg.set_payload(
                    IntoBytes::<authorization::AuthorizationMessage>::into_bytes(response)?,
                );

                sender
                    .send(context.source_id().clone(), msg.write_to_bytes()?)
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
    identity: String,
}

impl ConnectResponseHandler {
    fn new(identity: String) -> Self {
        ConnectResponseHandler { identity }
    }
}

impl Handler for ConnectResponseHandler {
    type Source = ConnectionId;
    type MessageType = authorization::AuthorizationMessageType;
    type Message = authorization::ConnectResponse;

    fn match_type(&self) -> Self::MessageType {
        authorization::AuthorizationMessageType::CONNECT_RESPONSE
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        let connect_response = ConnectResponse::from_proto(msg)?;
        debug!(
            "Receive connect response from connection {}: {:?}",
            context.source_connection_id(),
            connect_response,
        );

        if connect_response
            .accepted_authorization_types
            .iter()
            .any(|t| matches!(t, AuthorizationType::Trust))
        {
            let trust_request = AuthorizationMessage::TrustRequest(TrustRequest {
                identity: self.identity.clone(),
            });
            let mut msg = NetworkMessage::new();
            msg.set_message_type(NetworkMessageType::AUTHORIZATION);
            msg.set_payload(
                IntoBytes::<authorization::AuthorizationMessage>::into_bytes(trust_request)?,
            );
            sender
                .send(context.source_id().clone(), msg.write_to_bytes()?)
                .map_err(|(recipient, payload)| {
                    DispatchError::NetworkSendError((recipient.into(), payload))
                })?;
        }
        Ok(())
    }
}

/// Handler for the TrustRequest Authorization Message Type
struct TrustRequestHandler {
    auth_manager: AuthorizationManagerStateMachine,
}

impl TrustRequestHandler {
    fn new(auth_manager: AuthorizationManagerStateMachine) -> Self {
        TrustRequestHandler { auth_manager }
    }
}

impl Handler for TrustRequestHandler {
    type Source = ConnectionId;
    type MessageType = authorization::AuthorizationMessageType;
    type Message = authorization::TrustRequest;

    fn match_type(&self) -> Self::MessageType {
        authorization::AuthorizationMessageType::TRUST_REQUEST
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        let trust_request = TrustRequest::from_proto(msg)?;
        match self.auth_manager.next_state(
            context.source_connection_id(),
            AuthorizationAction::TrustIdentifying(trust_request.identity),
        ) {
            Err(err) => {
                debug!(
                    "Ignoring trust request message from connection {}: {}",
                    context.source_connection_id(),
                    err
                );
            }
            Ok(AuthorizationState::Authorized) => {
                debug!(
                    "Sending Authorized message to connection {}",
                    context.source_connection_id()
                );
                let auth_msg = AuthorizationMessage::Authorized(Authorized);
                let mut msg = NetworkMessage::new();
                msg.set_message_type(NetworkMessageType::AUTHORIZATION);
                msg.set_payload(
                    IntoBytes::<authorization::AuthorizationMessage>::into_bytes(auth_msg)?,
                );
                sender
                    .send(context.source_id().clone(), msg.write_to_bytes()?)
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
    auth_manager: AuthorizationManagerStateMachine,
}

impl AuthorizationErrorHandler {
    fn new(auth_manager: AuthorizationManagerStateMachine) -> Self {
        AuthorizationErrorHandler { auth_manager }
    }
}

impl Handler for AuthorizationErrorHandler {
    type Source = ConnectionId;
    type MessageType = authorization::AuthorizationMessageType;
    type Message = authorization::AuthorizationError;

    fn match_type(&self) -> Self::MessageType {
        authorization::AuthorizationMessageType::AUTHORIZATION_ERROR
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        _: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        let auth_error = AuthorizationError::from_proto(msg)?;
        match auth_error {
            AuthorizationError::AuthorizationRejected(err_msg) => {
                match self.auth_manager.next_state(
                    context.source_connection_id(),
                    AuthorizationAction::Unauthorizing,
                ) {
                    Ok(AuthorizationState::Unauthorized) => {
                        info!(
                            "Connection unauthorized by connection {}: {}",
                            context.source_connection_id(),
                            &err_msg
                        );
                    }
                    Err(err) => {
                        warn!(
                            "Unable to handle unauthorizing by connection {}: {}",
                            context.source_connection_id(),
                            err
                        );
                    }
                    Ok(next_state) => {
                        panic!("Should not have been able to transition to {}", next_state)
                    }
                }
            }
        }
        Ok(())
    }
}

impl MessageSender<ConnectionId> for AuthorizationMessageSender {
    fn send(&self, id: ConnectionId, message: Vec<u8>) -> Result<(), (ConnectionId, Vec<u8>)> {
        AuthorizationMessageSender::send(self, message).map_err(|msg| (id, msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use protobuf::Message;

    use crate::protos::authorization;
    use crate::protos::network::{NetworkMessage, NetworkMessageType};

    /// Test that an connect request is properly handled via the dispatcher.
    ///
    /// This is verified by:
    ///
    /// 1) no error from the dispatcher
    /// 2) the handler should send out two messages, a Unidirectional connect request and a connect
    ///    response.
    #[test]
    fn connect_request_dispatch() {
        let auth_mgr = AuthorizationManagerStateMachine::default();
        let mock_sender = MockSender::new();
        let dispatch_sender = mock_sender.clone();
        let dispatcher =
            create_authorization_dispatcher("mock_identity".into(), auth_mgr, dispatch_sender);

        let connection_id = "test_connection".to_string();
        let mut msg = authorization::ConnectRequest::new();
        msg.set_handshake_mode(authorization::ConnectRequest_HandshakeMode::BIDIRECTIONAL);
        let mut auth_msg = authorization::AuthorizationMessage::new();
        auth_msg.set_message_type(authorization::AuthorizationMessageType::CONNECT_REQUEST);
        auth_msg.set_payload(msg.write_to_bytes().unwrap());
        let msg_bytes = auth_msg.write_to_bytes().unwrap();

        assert_eq!(
            Ok(()),
            dispatcher.dispatch(
                connection_id.clone().into(),
                &NetworkMessageType::AUTHORIZATION,
                msg_bytes
            )
        );

        let (recipient, message_bytes) = mock_sender
            .next_outbound()
            .expect("Unable to receive message over the network");
        let recipient: String = recipient.into();
        assert_eq!(&connection_id, &recipient);
        let connect_req_msg: authorization::ConnectRequest = expect_auth_message(
            authorization::AuthorizationMessageType::CONNECT_REQUEST,
            &message_bytes,
        );
        assert_eq!(
            authorization::ConnectRequest_HandshakeMode::UNIDIRECTIONAL,
            connect_req_msg.get_handshake_mode()
        );

        let (_, message_bytes) = mock_sender
            .next_outbound()
            .expect("Unable to receive message over the network");

        let connect_res_msg: authorization::ConnectResponse = expect_auth_message(
            authorization::AuthorizationMessageType::CONNECT_RESPONSE,
            &message_bytes,
        );
        assert_eq!(
            vec![authorization::ConnectResponse_AuthorizationType::TRUST],
            connect_res_msg.get_accepted_authorization_types().to_vec()
        );
    }

    /// Test that a connect response is properly handled via the dispatcher.
    ///
    /// This is verified by:
    ///
    /// 1) a trust request is sent to the remote connection
    /// 2) the trust request includes the local identity
    #[test]
    fn connect_response_dispatch() {
        let auth_mgr = AuthorizationManagerStateMachine::default();
        let mock_sender = MockSender::new();
        let dispatch_sender = mock_sender.clone();
        let dispatcher =
            create_authorization_dispatcher("mock_identity".into(), auth_mgr, dispatch_sender);
        let connection_id = "test_connection".to_string();
        let mut msg = authorization::ConnectResponse::new();
        msg.set_accepted_authorization_types(
            vec![authorization::ConnectResponse_AuthorizationType::TRUST].into(),
        );
        let mut auth_msg = authorization::AuthorizationMessage::new();
        auth_msg.set_message_type(authorization::AuthorizationMessageType::CONNECT_RESPONSE);
        auth_msg.set_payload(msg.write_to_bytes().unwrap());
        let msg_bytes = auth_msg.write_to_bytes().unwrap();

        assert_eq!(
            Ok(()),
            dispatcher.dispatch(
                connection_id.clone().into(),
                &NetworkMessageType::AUTHORIZATION,
                msg_bytes
            )
        );

        let (_, msg_bytes) = mock_sender
            .next_outbound()
            .expect("Unable to receive message over the network");

        let trust_req: authorization::TrustRequest = expect_auth_message(
            authorization::AuthorizationMessageType::TRUST_REQUEST,
            &msg_bytes,
        );
        assert_eq!("mock_identity", trust_req.get_identity());
    }

    /// Test a trust request is properly handled via the dispatcher
    ///
    /// This is verified by:
    ///
    /// 1). sending a ConnectRequest, to get the state for the connection into the proper state
    /// 2). sending a TrustRequest, which would be the next step in authorization
    /// 3). receiving an Authorize message, which is the result of successful authorization
    #[test]
    fn trust_request_dispatch() {
        let auth_mgr = AuthorizationManagerStateMachine::default();
        let mock_sender = MockSender::new();
        let dispatch_sender = mock_sender.clone();
        let dispatcher =
            create_authorization_dispatcher("mock_identity".into(), auth_mgr, dispatch_sender);
        let connection_id = "test_connection".to_string();
        // Begin the connection process, otherwise, the response will fail
        let mut msg = authorization::ConnectRequest::new();
        msg.set_handshake_mode(authorization::ConnectRequest_HandshakeMode::UNIDIRECTIONAL);
        let mut auth_msg = authorization::AuthorizationMessage::new();
        auth_msg.set_message_type(authorization::AuthorizationMessageType::CONNECT_REQUEST);
        auth_msg.set_payload(msg.write_to_bytes().unwrap());

        let msg_bytes = auth_msg.write_to_bytes().unwrap();
        assert_eq!(
            Ok(()),
            dispatcher.dispatch(
                connection_id.clone().into(),
                &NetworkMessageType::AUTHORIZATION,
                msg_bytes
            )
        );

        let (_, msg_bytes) = mock_sender
            .next_outbound()
            .expect("Unable to receive message over the network");

        let _connect_res_msg: authorization::ConnectResponse = expect_auth_message(
            authorization::AuthorizationMessageType::CONNECT_RESPONSE,
            &msg_bytes,
        );

        let mut trust_req = authorization::TrustRequest::new();
        trust_req.set_identity("my_identity".into());
        let mut auth_msg = authorization::AuthorizationMessage::new();
        auth_msg.set_message_type(authorization::AuthorizationMessageType::TRUST_REQUEST);
        auth_msg.set_payload(trust_req.write_to_bytes().unwrap());
        let msg_bytes = auth_msg.write_to_bytes().unwrap();
        assert_eq!(
            Ok(()),
            dispatcher.dispatch(
                connection_id.clone().into(),
                &NetworkMessageType::AUTHORIZATION,
                msg_bytes
            )
        );

        let (_, msg_bytes) = mock_sender
            .next_outbound()
            .expect("Unable to receive message over the network");

        let _auth_msg: authorization::AuthorizedMessage = expect_auth_message(
            authorization::AuthorizationMessageType::AUTHORIZE,
            &msg_bytes,
        );
    }

    fn expect_auth_message<M: protobuf::Message>(
        message_type: authorization::AuthorizationMessageType,
        msg_bytes: &[u8],
    ) -> M {
        let network_msg: NetworkMessage =
            protobuf::parse_from_bytes(msg_bytes).expect("Unable to parse network message");
        assert_eq!(NetworkMessageType::AUTHORIZATION, network_msg.message_type);

        let auth_msg: authorization::AuthorizationMessage =
            protobuf::parse_from_bytes(network_msg.get_payload())
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

    #[derive(Clone)]
    struct MockSender {
        outbound: Arc<Mutex<VecDeque<(ConnectionId, Vec<u8>)>>>,
    }

    impl MockSender {
        fn new() -> Self {
            Self {
                outbound: Arc::new(Mutex::new(VecDeque::new())),
            }
        }

        fn next_outbound(&self) -> Option<(ConnectionId, Vec<u8>)> {
            self.outbound.lock().expect("lock was poisoned").pop_front()
        }
    }

    impl MessageSender<ConnectionId> for MockSender {
        fn send(&self, id: ConnectionId, message: Vec<u8>) -> Result<(), (ConnectionId, Vec<u8>)> {
            self.outbound
                .lock()
                .expect("lock was poisoned")
                .push_back((id, message));

            Ok(())
        }
    }
}
