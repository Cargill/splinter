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

use crate::error::InternalError;
use crate::network::dispatch::{
    ConnectionId, DispatchError, Handler, MessageContext, MessageSender,
};
use crate::protocol::authorization::{
    AuthorizationMessage, AuthorizationType, Authorized, ConnectRequest, ConnectResponse,
    TrustRequest,
};
use crate::protocol::network::NetworkMessage;
use crate::protos::authorization;
use crate::protos::network;
use crate::protos::prelude::*;

use crate::network::auth::{
    state_machine::trust_v0::{TrustV0AuthorizationAction, TrustV0AuthorizationState},
    AuthorizationAcceptingAction, AuthorizationAcceptingState, AuthorizationActionError,
    AuthorizationManagerStateMachine, Identity,
};

pub struct AuthorizedHandler {
    auth_manager: AuthorizationManagerStateMachine,
}

impl AuthorizedHandler {
    pub fn new(auth_manager: AuthorizationManagerStateMachine) -> Self {
        Self { auth_manager }
    }
}

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
        debug!(
            "Received authorize message from {}",
            context.source_connection_id()
        );
        match self.auth_manager.next_accepting_state(
            context.source_connection_id(),
            AuthorizationAcceptingAction::TrustV0(TrustV0AuthorizationAction::RemoteAuthorizing),
        ) {
            Err(err) => {
                warn!(
                    "Ignoring authorize message from {}: {}",
                    context.source_connection_id(),
                    err
                );
            }

            Ok(_) => {
                debug!("Authorized by {}", context.source_connection_id());
            }
        }

        Ok(())
    }
}

///
/// Handler for the Connect Request Authorization Message Type
pub struct ConnectRequestHandler {
    auth_manager: AuthorizationManagerStateMachine,
}

impl ConnectRequestHandler {
    pub fn new(auth_manager: AuthorizationManagerStateMachine) -> Self {
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
        match self.auth_manager.next_accepting_state(
            context.source_connection_id(),
            AuthorizationAcceptingAction::Connecting,
        ) {
            Err(AuthorizationActionError::AlreadyConnecting) => {
                debug!(
                    "Ignoring duplicate connect request from {}",
                    context.source_connection_id(),
                );
            }
            Err(err) => {
                warn!(
                    "Ignoring connect message from {}: {}",
                    context.source_connection_id(),
                    err
                );
            }
            Ok(AuthorizationAcceptingState::TrustV0(TrustV0AuthorizationState::Connecting)) => {
                debug!("Beginning handshake for {}", context.source_connection_id(),);
                // Send a connect request of our own

                match connect_request {
                    ConnectRequest::Bidirectional => {
                        let connect_req =
                            AuthorizationMessage::ConnectRequest(ConnectRequest::Unidirectional);
                        let msg_bytes = IntoBytes::<network::NetworkMessage>::into_bytes(
                            NetworkMessage::from(connect_req),
                        )?;
                        sender
                            .send(context.source_id().clone(), msg_bytes)
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

                let msg_bytes = IntoBytes::<network::NetworkMessage>::into_bytes(
                    NetworkMessage::from(response),
                )?;

                sender
                    .send(context.source_id().clone(), msg_bytes)
                    .map_err(|(recipient, payload)| {
                        DispatchError::NetworkSendError((recipient.into(), payload))
                    })?;
            }
            Ok(next_state) => {
                return Err(DispatchError::InternalError(InternalError::with_message(
                    format!("Should not have been able to transition to {}", next_state),
                )))
            }
        }

        Ok(())
    }
}

/// Handler for the ConnectResponse Authorization Message Type
pub struct ConnectResponseHandler {
    identity: String,
    auth_manager: AuthorizationManagerStateMachine,
}

impl ConnectResponseHandler {
    pub fn new(identity: String, auth_manager: AuthorizationManagerStateMachine) -> Self {
        ConnectResponseHandler {
            identity,
            auth_manager,
        }
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

        self.auth_manager
            .set_local_authorization(
                context.source_connection_id(),
                Identity::Trust {
                    identity: self.identity.to_string(),
                },
            )
            .map_err(|err| {
                DispatchError::HandleError(format!("Unable to set local authorization: {}", err))
            })?;

        if connect_response
            .accepted_authorization_types
            .iter()
            .any(|t| matches!(t, AuthorizationType::Trust))
        {
            let trust_request = AuthorizationMessage::TrustRequest(TrustRequest {
                identity: self.identity.clone(),
            });
            let msg_bytes = IntoBytes::<network::NetworkMessage>::into_bytes(
                NetworkMessage::from(trust_request),
            )?;
            sender
                .send(context.source_id().clone(), msg_bytes)
                .map_err(|(recipient, payload)| {
                    DispatchError::NetworkSendError((recipient.into(), payload))
                })?;
        }
        Ok(())
    }
}

/// Handler for the TrustRequest Authorization Message Type
pub struct TrustRequestHandler {
    auth_manager: AuthorizationManagerStateMachine,
}

impl TrustRequestHandler {
    pub fn new(auth_manager: AuthorizationManagerStateMachine) -> Self {
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
        match self.auth_manager.next_accepting_state(
            context.source_connection_id(),
            AuthorizationAcceptingAction::TrustV0(TrustV0AuthorizationAction::TrustIdentifyingV0(
                Identity::Trust {
                    identity: trust_request.identity,
                },
            )),
        ) {
            Err(err) => {
                warn!(
                    "Ignoring trust request message from connection {}: {}",
                    context.source_connection_id(),
                    err
                );
            }
            Ok(AuthorizationAcceptingState::TrustV0(
                TrustV0AuthorizationState::RemoteIdentified(Identity::Trust { identity }),
            ))
            | Ok(AuthorizationAcceptingState::Done(Identity::Trust { identity })) => {
                debug!(
                    "Sending Authorized message to connection {} after receiving identity {}",
                    context.source_connection_id(),
                    identity
                );
                let auth_msg = AuthorizationMessage::Authorized(Authorized);
                let msg_bytes = IntoBytes::<network::NetworkMessage>::into_bytes(
                    NetworkMessage::from(auth_msg),
                )?;
                sender
                    .send(context.source_id().clone(), msg_bytes)
                    .map_err(|(recipient, payload)| {
                        DispatchError::NetworkSendError((recipient.into(), payload))
                    })?;
            }
            Ok(next_state) => {
                return Err(DispatchError::InternalError(InternalError::with_message(
                    format!("Should not have been able to transition to {}", next_state),
                )))
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    #[cfg(feature = "challenge-authorization")]
    use cylinder::{secp256k1::Secp256k1Context, Context, Signer};
    use cylinder::{PublicKey, Signature, VerificationError, Verifier};
    use protobuf::Message;

    use crate::network::auth::AuthorizationDispatchBuilder;
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
        // mut is required if chalenge authorization is enabled
        #[allow(unused_mut)]
        let mut dispatcher_builder =
            AuthorizationDispatchBuilder::new().with_identity("mock_identity");

        #[cfg(feature = "challenge-authorization")]
        {
            dispatcher_builder = dispatcher_builder
                .with_signers(&vec![new_signer()])
                .with_nonce(&vec![])
                .with_expected_authorization(None)
                .with_local_authorization(None)
                .with_verifier(Box::new(NoopVerifier))
        }

        let dispatcher = dispatcher_builder
            .build(dispatch_sender, auth_mgr)
            .expect("Unable to build authorization dispatcher");

        let connection_id = "test_connection".to_string();
        let mut msg = authorization::ConnectRequest::new();
        msg.set_handshake_mode(authorization::ConnectRequest_HandshakeMode::BIDIRECTIONAL);
        let mut auth_msg = authorization::AuthorizationMessage::new();
        auth_msg.set_message_type(authorization::AuthorizationMessageType::CONNECT_REQUEST);
        auth_msg.set_payload(msg.write_to_bytes().unwrap());
        let msg_bytes = auth_msg.write_to_bytes().unwrap();

        assert!(dispatcher
            .dispatch(
                connection_id.clone().into(),
                &NetworkMessageType::AUTHORIZATION,
                msg_bytes
            )
            .is_ok());

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
        // mut is required if chalenge authorization is enabled
        #[allow(unused_mut)]
        let mut dispatcher_builder =
            AuthorizationDispatchBuilder::new().with_identity("mock_identity");

        #[cfg(feature = "challenge-authorization")]
        {
            dispatcher_builder = dispatcher_builder
                .with_signers(&vec![new_signer()])
                .with_nonce(&vec![])
                .with_expected_authorization(None)
                .with_local_authorization(None)
                .with_verifier(Box::new(NoopVerifier))
        }

        let dispatcher = dispatcher_builder
            .build(dispatch_sender, auth_mgr)
            .expect("Unable to build authorization dispatcher");
        let connection_id = "test_connection".to_string();
        let mut msg = authorization::ConnectResponse::new();
        msg.set_accepted_authorization_types(
            vec![authorization::ConnectResponse_AuthorizationType::TRUST].into(),
        );
        let mut auth_msg = authorization::AuthorizationMessage::new();
        auth_msg.set_message_type(authorization::AuthorizationMessageType::CONNECT_RESPONSE);
        auth_msg.set_payload(msg.write_to_bytes().unwrap());
        let msg_bytes = auth_msg.write_to_bytes().unwrap();

        assert!(dispatcher
            .dispatch(
                connection_id.clone().into(),
                &NetworkMessageType::AUTHORIZATION,
                msg_bytes
            )
            .is_ok());

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
        // mut is required if chalenge authorization is enabled
        #[allow(unused_mut)]
        let mut dispatcher_builder =
            AuthorizationDispatchBuilder::new().with_identity("mock_identity");

        #[cfg(feature = "challenge-authorization")]
        {
            dispatcher_builder = dispatcher_builder
                .with_signers(&vec![new_signer()])
                .with_nonce(&vec![])
                .with_expected_authorization(None)
                .with_local_authorization(None)
                .with_verifier(Box::new(NoopVerifier))
        }

        let dispatcher = dispatcher_builder
            .build(dispatch_sender, auth_mgr)
            .expect("Unable to build authorization dispatcher");
        let connection_id = "test_connection".to_string();
        // Begin the connection process, otherwise, the response will fail
        let mut msg = authorization::ConnectRequest::new();
        msg.set_handshake_mode(authorization::ConnectRequest_HandshakeMode::UNIDIRECTIONAL);
        let mut auth_msg = authorization::AuthorizationMessage::new();
        auth_msg.set_message_type(authorization::AuthorizationMessageType::CONNECT_REQUEST);
        auth_msg.set_payload(msg.write_to_bytes().unwrap());

        let msg_bytes = auth_msg.write_to_bytes().unwrap();
        assert!(dispatcher
            .dispatch(
                connection_id.clone().into(),
                &NetworkMessageType::AUTHORIZATION,
                msg_bytes
            )
            .is_ok());

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
        assert!(dispatcher
            .dispatch(
                connection_id.clone().into(),
                &NetworkMessageType::AUTHORIZATION,
                msg_bytes
            )
            .is_ok());

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
            Message::parse_from_bytes(msg_bytes).expect("Unable to parse network message");
        assert_eq!(NetworkMessageType::AUTHORIZATION, network_msg.message_type);

        let auth_msg: authorization::AuthorizationMessage =
            Message::parse_from_bytes(network_msg.get_payload())
                .expect("Unable to parse auth message");

        assert_eq!(message_type, auth_msg.message_type);

        match Message::parse_from_bytes(auth_msg.get_payload()) {
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

    struct NoopVerifier;

    impl Verifier for NoopVerifier {
        fn algorithm_name(&self) -> &str {
            unimplemented!()
        }

        fn verify(
            &self,
            _message: &[u8],
            _signature: &Signature,
            _public_key: &PublicKey,
        ) -> Result<bool, VerificationError> {
            unimplemented!()
        }
    }

    #[cfg(feature = "challenge-authorization")]
    fn new_signer() -> Box<dyn Signer> {
        let context = Secp256k1Context::new();
        let key = context.new_random_private_key();
        context.new_signer(key)
    }
}
