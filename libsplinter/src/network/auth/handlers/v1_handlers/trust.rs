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

//! Message handlers for Trust v1 authorization messages

use crate::network::auth::state_machine::trust_v1::{
    TrustAuthorizationLocalAction, TrustAuthorizationRemoteAction, TrustAuthorizationRemoteState,
};
use crate::network::auth::{
    AuthorizationLocalAction, AuthorizationLocalState, AuthorizationManagerStateMachine,
    AuthorizationMessage, AuthorizationRemoteAction, AuthorizationRemoteState, Identity,
};
use crate::network::dispatch::{
    ConnectionId, DispatchError, Handler, MessageContext, MessageSender,
};
use crate::protocol::authorization::AuthComplete;
use crate::protocol::authorization::{AuthTrustRequest, AuthTrustResponse};
use crate::protocol::network::NetworkMessage;
use crate::protos::authorization;
use crate::protos::network;
use crate::protos::prelude::*;

use super::send_authorization_error;

/// Handler for the Authorization Trust Request Message Type
pub struct AuthTrustRequestHandler {
    auth_manager: AuthorizationManagerStateMachine,
}

impl AuthTrustRequestHandler {
    pub fn new(auth_manager: AuthorizationManagerStateMachine) -> Self {
        Self { auth_manager }
    }
}

impl Handler for AuthTrustRequestHandler {
    type Source = ConnectionId;
    type MessageType = authorization::AuthorizationMessageType;
    type Message = authorization::AuthTrustRequest;

    fn match_type(&self) -> Self::MessageType {
        authorization::AuthorizationMessageType::AUTH_TRUST_REQUEST
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        debug!(
            "Received authorization trust request from {}",
            context.source_connection_id()
        );
        let trust_request = AuthTrustRequest::from_proto(msg)?;
        match self.auth_manager.next_remote_state(
            context.source_connection_id(),
            AuthorizationRemoteAction::Trust(
                TrustAuthorizationRemoteAction::ReceiveAuthTrustRequest(Identity::Trust {
                    identity: trust_request.identity.to_string(),
                }),
            ),
        ) {
            Err(err) => {
                send_authorization_error(
                    &self.auth_manager,
                    context.source_id(),
                    context.source_connection_id(),
                    sender,
                    &err.to_string(),
                )?;
                return Ok(());
            }
            Ok(AuthorizationRemoteState::Trust(
                TrustAuthorizationRemoteState::ReceivedAuthTrustRequest(_),
            )) => {
                debug!(
                    "Sending trust response to connection {} after receiving identity {}",
                    context.source_connection_id(),
                    trust_request.identity,
                );
                let auth_msg = AuthorizationMessage::AuthTrustResponse(AuthTrustResponse);
                let msg_bytes = IntoBytes::<network::NetworkMessage>::into_bytes(
                    NetworkMessage::from(auth_msg),
                )?;
                sender
                    .send(context.source_id().clone(), msg_bytes)
                    .map_err(|(recipient, payload)| {
                        DispatchError::NetworkSendError((recipient.into(), payload))
                    })?;
            }
            Ok(next_state) => panic!("Should not have been able to transition to {}", next_state),
        }

        if self
            .auth_manager
            .next_remote_state(
                context.source_connection_id(),
                AuthorizationRemoteAction::Trust(
                    TrustAuthorizationRemoteAction::SendAuthTrustResponse,
                ),
            )
            .is_err()
        {
            error!("Unable to transition from ReceivedAuthTrustRequest to Done")
        };

        Ok(())
    }
}

/// Handler for the Authorization Trust Response Message Type
pub struct AuthTrustResponseHandler {
    auth_manager: AuthorizationManagerStateMachine,
}

impl AuthTrustResponseHandler {
    pub fn new(auth_manager: AuthorizationManagerStateMachine) -> Self {
        Self { auth_manager }
    }
}

impl Handler for AuthTrustResponseHandler {
    type Source = ConnectionId;
    type MessageType = authorization::AuthorizationMessageType;
    type Message = authorization::AuthTrustResponse;

    fn match_type(&self) -> Self::MessageType {
        authorization::AuthorizationMessageType::AUTH_TRUST_RESPONSE
    }

    fn handle(
        &self,
        _msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        debug!(
            "Received authorization trust response from {}",
            context.source_connection_id()
        );
        match self.auth_manager.next_local_state(
            context.source_connection_id(),
            AuthorizationLocalAction::Trust(
                TrustAuthorizationLocalAction::ReceiveAuthTrustResponse,
            ),
        ) {
            Err(err) => {
                send_authorization_error(
                    &self.auth_manager,
                    context.source_id(),
                    context.source_connection_id(),
                    sender,
                    &err.to_string(),
                )?;
                return Ok(());
            }
            Ok(AuthorizationLocalState::Authorized) => (),
            Ok(next_state) => panic!("Should not have been able to transition to {}", next_state),
        }

        let auth_msg = AuthorizationMessage::AuthComplete(AuthComplete);
        let msg_bytes =
            IntoBytes::<network::NetworkMessage>::into_bytes(NetworkMessage::from(auth_msg))?;
        sender
            .send(context.source_id().clone(), msg_bytes)
            .map_err(|(recipient, payload)| {
                DispatchError::NetworkSendError((recipient.into(), payload))
            })?;

        match self.auth_manager.next_local_state(
            context.source_connection_id(),
            AuthorizationLocalAction::SendAuthComplete,
        ) {
            Err(err) => {
                send_authorization_error(
                    &self.auth_manager,
                    context.source_id(),
                    context.source_connection_id(),
                    sender,
                    &err.to_string(),
                )?;
                return Ok(());
            }
            Ok(AuthorizationLocalState::WaitForComplete) => (),
            Ok(AuthorizationLocalState::AuthorizedAndComplete) => (),
            Ok(next_state) => panic!("Should not have been able to transition to {}", next_state),
        };

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use cylinder::{
        secp256k1::Secp256k1Context, Context, PublicKey, Signature, Signer, VerificationError,
        Verifier,
    };
    use protobuf::Message;

    use crate::network::auth::state_machine::trust_v1::TrustAuthorizationLocalState;
    use crate::network::auth::AuthorizationDispatchBuilder;
    use crate::network::auth::ManagedAuthorizationState;
    use crate::protocol::authorization::{AuthProtocolResponse, PeerAuthorizationType};
    use crate::protos::network::NetworkMessageType;
    use crate::protos::{authorization, network};

    /// Test that an auth protocol response is properly handled via the dispatcher when trust is
    /// is set as the accepted authorization type.
    ///
    /// This is verified by:
    ///
    /// 1) no error from the dispatcher
    /// 2) the handler should send an AuthTrustRequest with the provided identity
    #[test]
    fn protocol_response_trust() {
        let connection_id = "test_connection".to_string();
        // need to setup expected authorization state
        let auth_mgr = AuthorizationManagerStateMachine::default();
        auth_mgr
            .shared
            .lock()
            .expect("lock poisoned")
            .states
            .insert(
                connection_id.to_string(),
                ManagedAuthorizationState {
                    local_state: AuthorizationLocalState::WaitingForAuthProtocolResponse,
                    remote_state: AuthorizationRemoteState::SentAuthProtocolResponse,
                    received_complete: false,
                },
            );
        let mock_sender = MockSender::new();
        let dispatch_sender = mock_sender.clone();
        // mut is required if chalenge authorization is enabled
        #[allow(unused_mut)]
        let mut dispatcher_builder =
            AuthorizationDispatchBuilder::new().with_identity("mock_identity");

        #[cfg(feature = "challenge-authorization")]
        {
            dispatcher_builder = dispatcher_builder
                .with_signers(&[new_signer()])
                .with_nonce(&vec![])
                .with_expected_authorization(None)
                .with_local_authorization(None)
                .with_verifier(Box::new(NoopVerifier))
        }

        let dispatcher = dispatcher_builder
            .build(dispatch_sender, auth_mgr)
            .expect("Unable to build authorization dispatcher");

        let msg_bytes = IntoBytes::<authorization::AuthorizationMessage>::into_bytes(
            AuthorizationMessage::AuthProtocolResponse(AuthProtocolResponse {
                auth_protocol: 1,
                accepted_authorization_type: vec![PeerAuthorizationType::Trust],
            }),
        )
        .expect("Unable to get message bytes");

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

        let trust_request: authorization::AuthTrustRequest = expect_auth_message(
            authorization::AuthorizationMessageType::AUTH_TRUST_REQUEST,
            &message_bytes,
        );
        assert_eq!("mock_identity", trust_request.get_identity());
    }

    /// Test that a trust request is properly handled. Also verify end state is set to
    /// WaitingForAuthTrustResponse and Done.
    ///
    /// This is verified by:
    ///
    /// 1) no error from the dispatcher
    /// 2) the handler should send AuthTrustResponse Message
    /// 3) verify the states are set to WaitingForAuthTrustResponse and Done(identity)
    #[test]
    fn trust_request() {
        let connection_id = "test_connection".to_string();
        // need to setup expected authorization state
        let auth_mgr = AuthorizationManagerStateMachine::default();
        auth_mgr
            .shared
            .lock()
            .expect("lock poisoned")
            .states
            .insert(
                connection_id.to_string(),
                ManagedAuthorizationState {
                    local_state: AuthorizationLocalState::Trust(
                        TrustAuthorizationLocalState::WaitingForAuthTrustResponse,
                    ),
                    remote_state: AuthorizationRemoteState::SentAuthProtocolResponse,
                    received_complete: false,
                },
            );
        let mock_sender = MockSender::new();
        let dispatch_sender = mock_sender.clone();
        // mut is required if chalenge authorization is enabled
        #[allow(unused_mut)]
        let mut dispatcher_builder =
            AuthorizationDispatchBuilder::new().with_identity("mock_identity");

        #[cfg(feature = "challenge-authorization")]
        {
            dispatcher_builder = dispatcher_builder
                .with_signers(&[new_signer()])
                .with_nonce(&vec![])
                .with_expected_authorization(None)
                .with_local_authorization(None)
                .with_verifier(Box::new(NoopVerifier))
        }

        let dispatcher = dispatcher_builder
            .build(dispatch_sender, auth_mgr.clone())
            .expect("Unable to build authorization dispatcher");

        let msg_bytes = IntoBytes::<authorization::AuthorizationMessage>::into_bytes(
            AuthorizationMessage::AuthTrustRequest(AuthTrustRequest {
                identity: "other_identity".to_string(),
            }),
        )
        .expect("Unable to get message bytes");

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

        let _trust_response: authorization::AuthTrustResponse = expect_auth_message(
            authorization::AuthorizationMessageType::AUTH_TRUST_RESPONSE,
            &message_bytes,
        );

        let managed_state = auth_mgr
            .shared
            .lock()
            .expect("lock poisoned")
            .states
            .get(&connection_id)
            .cloned()
            .expect("missing managed state for connection id");

        assert_eq!(
            managed_state.local_state,
            AuthorizationLocalState::Trust(
                TrustAuthorizationLocalState::WaitingForAuthTrustResponse
            )
        );
        assert_eq!(
            managed_state.remote_state,
            AuthorizationRemoteState::Done(Identity::Trust {
                identity: "other_identity".to_string()
            })
        );
        assert_eq!(managed_state.received_complete, false);
    }

    /// Test that a trust response is properly handled. Also verify end state is set to
    /// WaitForComplete because received_complete is set to false
    ///
    /// This is verified by:
    ///
    /// 1) no error from the dispatcher
    /// 2) the handler should send AuthComplete Message
    /// 3) verify that because auth complete has not been received, the states are set to
    ///    WaitingForComplete and Done(identity)
    #[test]
    fn trust_response() {
        let connection_id = "test_connection".to_string();
        // need to setup expected authorization state
        let auth_mgr = AuthorizationManagerStateMachine::default();
        auth_mgr
            .shared
            .lock()
            .expect("lock poisoned")
            .states
            .insert(
                connection_id.to_string(),
                ManagedAuthorizationState {
                    local_state: AuthorizationLocalState::Trust(
                        TrustAuthorizationLocalState::WaitingForAuthTrustResponse,
                    ),
                    remote_state: AuthorizationRemoteState::Done(Identity::Trust {
                        identity: "other_identity".to_string(),
                    }),
                    received_complete: false,
                },
            );
        let mock_sender = MockSender::new();
        let dispatch_sender = mock_sender.clone();
        // mut is required if chalenge authorization is enabled
        #[allow(unused_mut)]
        let mut dispatcher_builder =
            AuthorizationDispatchBuilder::new().with_identity("mock_identity");

        #[cfg(feature = "challenge-authorization")]
        {
            dispatcher_builder = dispatcher_builder
                .with_signers(&[new_signer()])
                .with_nonce(&vec![])
                .with_expected_authorization(None)
                .with_local_authorization(None)
                .with_verifier(Box::new(NoopVerifier))
        }

        let dispatcher = dispatcher_builder
            .build(dispatch_sender, auth_mgr.clone())
            .expect("Unable to build authorization dispatcher");

        let msg_bytes = IntoBytes::<authorization::AuthorizationMessage>::into_bytes(
            AuthorizationMessage::AuthTrustResponse(AuthTrustResponse),
        )
        .expect("Unable to get message bytes");

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

        let _trust_response: authorization::AuthComplete = expect_auth_message(
            authorization::AuthorizationMessageType::AUTH_COMPLETE,
            &message_bytes,
        );

        let managed_state = auth_mgr
            .shared
            .lock()
            .expect("lock poisoned")
            .states
            .get(&connection_id)
            .cloned()
            .expect("missing managed state for connection id");

        assert_eq!(
            managed_state.local_state,
            AuthorizationLocalState::WaitForComplete,
        );
        assert_eq!(
            managed_state.remote_state,
            AuthorizationRemoteState::Done(Identity::Trust {
                identity: "other_identity".to_string()
            })
        );
        assert_eq!(managed_state.received_complete, false);
    }

    /// Test that a trust response is properly handled. Also verify end state is set to
    /// AuthorizedAndComplete because received_complete is set to true
    ///
    /// This is verified by:
    ///
    /// 1) no error from the dispatcher
    /// 2) the handler should send AuthComplete Message
    /// 3) verify that because auth complete has  been received, the states are set to
    ///    AuthorizedAndComplete and Done(identity)
    #[test]
    fn trust_response_complete() {
        let connection_id = "test_connection".to_string();
        // need to setup expected authorization state
        let auth_mgr = AuthorizationManagerStateMachine::default();
        auth_mgr
            .shared
            .lock()
            .expect("lock poisoned")
            .states
            .insert(
                connection_id.to_string(),
                ManagedAuthorizationState {
                    local_state: AuthorizationLocalState::Trust(
                        TrustAuthorizationLocalState::WaitingForAuthTrustResponse,
                    ),
                    remote_state: AuthorizationRemoteState::Done(Identity::Trust {
                        identity: "other_identity".to_string(),
                    }),
                    received_complete: true,
                },
            );
        let mock_sender = MockSender::new();
        let dispatch_sender = mock_sender.clone();
        // mut is required if chalenge authorization is enabled
        #[allow(unused_mut)]
        let mut dispatcher_builder =
            AuthorizationDispatchBuilder::new().with_identity("mock_identity");

        #[cfg(feature = "challenge-authorization")]
        {
            dispatcher_builder = dispatcher_builder
                .with_signers(&[new_signer()])
                .with_nonce(&vec![])
                .with_expected_authorization(None)
                .with_local_authorization(None)
                .with_verifier(Box::new(NoopVerifier))
        }

        let dispatcher = dispatcher_builder
            .build(dispatch_sender, auth_mgr.clone())
            .expect("Unable to build authorization dispatcher");

        let msg_bytes = IntoBytes::<authorization::AuthorizationMessage>::into_bytes(
            AuthorizationMessage::AuthTrustResponse(AuthTrustResponse),
        )
        .expect("Unable to get message bytes");

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

        let _trust_response: authorization::AuthComplete = expect_auth_message(
            authorization::AuthorizationMessageType::AUTH_COMPLETE,
            &message_bytes,
        );

        let managed_state = auth_mgr
            .shared
            .lock()
            .expect("lock poisoned")
            .states
            .get(&connection_id)
            .cloned()
            .expect("missing managed state for connection id");

        assert_eq!(
            managed_state.local_state,
            AuthorizationLocalState::AuthorizedAndComplete,
        );
        assert_eq!(
            managed_state.remote_state,
            AuthorizationRemoteState::Done(Identity::Trust {
                identity: "other_identity".to_string()
            })
        );
        assert_eq!(managed_state.received_complete, true);
    }

    fn expect_auth_message<M: protobuf::Message>(
        message_type: authorization::AuthorizationMessageType,
        msg_bytes: &[u8],
    ) -> M {
        let network_msg: network::NetworkMessage =
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
            Ok(true)
        }
    }

    fn new_signer() -> Box<dyn Signer> {
        let context = Secp256k1Context::new();
        let key = context.new_random_private_key();
        context.new_signer(key)
    }
}
