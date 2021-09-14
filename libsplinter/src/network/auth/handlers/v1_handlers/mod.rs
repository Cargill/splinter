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

//! Message handlers for v1 authorization messages

pub mod builders;
#[cfg(feature = "challenge-authorization")]
pub mod challenge;
#[cfg(feature = "trust-authorization")]
pub mod trust;

#[cfg(feature = "challenge-authorization")]
use crate::network::auth::state_machine::challenge_v1::ChallengeAuthorizationLocalAction;
#[cfg(feature = "trust-authorization")]
use crate::network::auth::state_machine::trust_v1::TrustAuthorizationLocalAction;
#[cfg(feature = "trust-authorization")]
use crate::network::auth::Identity;
use crate::network::auth::{
    AuthorizationLocalAction, AuthorizationLocalState, AuthorizationManagerStateMachine,
    AuthorizationMessage, AuthorizationRemoteAction, AuthorizationRemoteState,
    ConnectionAuthorizationType,
};
use crate::network::dispatch::{
    ConnectionId, DispatchError, Handler, MessageContext, MessageSender,
};
#[cfg(feature = "challenge-authorization")]
use crate::protocol::authorization::AuthChallengeNonceRequest;
#[cfg(feature = "trust-authorization")]
use crate::protocol::authorization::AuthTrustRequest;
use crate::protocol::authorization::{
    AuthProtocolRequest, AuthProtocolResponse, AuthorizationError, PeerAuthorizationType,
};
use crate::protocol::network::NetworkMessage;
use crate::protocol::{PEER_AUTHORIZATION_PROTOCOL_MIN, PEER_AUTHORIZATION_PROTOCOL_VERSION};
use crate::protos::authorization;
use crate::protos::network;
use crate::protos::prelude::*;

/// Handler for the Authorization Protocol Request Message Type
pub struct AuthProtocolRequestHandler {
    auth_manager: AuthorizationManagerStateMachine,
    #[cfg(feature = "challenge-authorization")]
    expected_authorization: Option<ConnectionAuthorizationType>,
    #[cfg(feature = "challenge-authorization")]
    local_authorization: Option<ConnectionAuthorizationType>,
}

impl Handler for AuthProtocolRequestHandler {
    type Source = ConnectionId;
    type MessageType = authorization::AuthorizationMessageType;
    type Message = authorization::AuthProtocolRequest;

    fn match_type(&self) -> Self::MessageType {
        authorization::AuthorizationMessageType::AUTH_PROTOCOL_REQUEST
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        debug!(
            "Received authorization protocol request from {}",
            context.source_connection_id()
        );
        let protocol_request = AuthProtocolRequest::from_proto(msg)?;

        match self.auth_manager.next_remote_state(
            context.source_connection_id(),
            AuthorizationRemoteAction::ReceiveAuthProtocolRequest,
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

            Ok(AuthorizationRemoteState::ReceivedAuthProtocolRequest) => {
                let version = supported_protocol_version(
                    protocol_request.auth_protocol_min,
                    protocol_request.auth_protocol_max,
                );

                // Send error message if version is not agreed upon
                if version == 0 {
                    send_authorization_error(
                        &self.auth_manager,
                        context.source_id(),
                        context.source_connection_id(),
                        sender,
                        "Unable to agree on protocol version",
                    )?;
                    return Ok(());
                };

                debug!(
                    "Sending agreed upon protocol version: {} and authorization types",
                    version
                );

                let mut accepted_authorization_type = vec![];
                #[cfg(feature = "trust-authorization")]
                {
                    accepted_authorization_type.push(PeerAuthorizationType::Trust);
                }

                // If expected_authorization type is set, that means we are the side that has
                // circuit/proposal and we need to make sure that we only send the authorization
                // type that is required, otherwise the other side (which does not yet have a
                // circuit/proposal information) could choose the wrong type of authorization. If
                // we do not have an expected authorization type we want to include all of the
                // supported authorization types so the other side can make the decision on what
                // type of authorization to do.
                #[cfg(feature = "challenge-authorization")]
                match self.expected_authorization {
                    #[cfg(feature = "trust-authorization")]
                    Some(ConnectionAuthorizationType::Trust { .. }) => (),
                    Some(ConnectionAuthorizationType::Challenge { .. }) => {
                        accepted_authorization_type = vec![PeerAuthorizationType::Challenge]
                    }
                    // if None, check required local authorization type as well
                    _ => {
                        match self.local_authorization {
                            #[cfg(feature = "trust-authorization")]
                            Some(ConnectionAuthorizationType::Trust { .. }) => (),
                            Some(ConnectionAuthorizationType::Challenge { .. }) => {
                                accepted_authorization_type = vec![PeerAuthorizationType::Challenge]
                            }
                            _ => {
                                // if trust is enabled it was already added
                                accepted_authorization_type.push(PeerAuthorizationType::Challenge)
                            }
                        }
                    }
                };

                let response = AuthorizationMessage::AuthProtocolResponse(AuthProtocolResponse {
                    auth_protocol: version,
                    accepted_authorization_type,
                });

                let msg_bytes = IntoBytes::<network::NetworkMessage>::into_bytes(
                    NetworkMessage::from(response),
                )?;

                sender
                    .send(context.source_id().clone(), msg_bytes)
                    .map_err(|(recipient, payload)| {
                        DispatchError::NetworkSendError((recipient.into(), payload))
                    })?;

                if self
                    .auth_manager
                    .next_remote_state(
                        context.source_connection_id(),
                        AuthorizationRemoteAction::SendAuthProtocolResponse,
                    )
                    .is_err()
                {
                    error!(
                        "Unable to transition from ReceivedAuthProtocolRequest to \
                        SentAuthProtocolResponse"
                    )
                };
            }
            Ok(next_state) => panic!("Should not have been able to transition to {}", next_state),
        }
        Ok(())
    }
}

/// Return the supported protocol version that matches the min/max provided. If there is no
/// matching protocol version 0 is returned.
fn supported_protocol_version(min: u32, max: u32) -> u32 {
    if max < min {
        info!("Received invalid peer auth protocol request: min cannot be greater than max");
        return 0;
    }

    if min > PEER_AUTHORIZATION_PROTOCOL_VERSION {
        info!(
            "Request requires newer version than can be provided: {}",
            min
        );
        return 0;
    } else if max < PEER_AUTHORIZATION_PROTOCOL_MIN {
        info!(
            "Request requires older version than can be provided: {}",
            max
        );
        return 0;
    }

    if max >= PEER_AUTHORIZATION_PROTOCOL_VERSION {
        PEER_AUTHORIZATION_PROTOCOL_VERSION
    } else if max > PEER_AUTHORIZATION_PROTOCOL_MIN {
        max
    } else if min > PEER_AUTHORIZATION_PROTOCOL_MIN {
        min
    } else {
        PEER_AUTHORIZATION_PROTOCOL_MIN
    }
}

/// Handler for the Authorization Protocol Response Message Type
pub struct AuthProtocolResponseHandler {
    auth_manager: AuthorizationManagerStateMachine,
    #[cfg(feature = "trust-authorization")]
    identity: String,
    required_local_auth: Option<ConnectionAuthorizationType>,
}

impl Handler for AuthProtocolResponseHandler {
    type Source = ConnectionId;
    type MessageType = authorization::AuthorizationMessageType;
    type Message = authorization::AuthProtocolResponse;

    fn match_type(&self) -> Self::MessageType {
        authorization::AuthorizationMessageType::AUTH_PROTOCOL_RESPONSE
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        debug!(
            "Received authorization protocol response from {}",
            context.source_connection_id()
        );

        let protocol_request = AuthProtocolResponse::from_proto(msg)?;

        let mut msg_bytes = vec![];
        match self.auth_manager.next_local_state(
            context.source_connection_id(),
            AuthorizationLocalAction::ReceiveAuthProtocolResponse,
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
            Ok(AuthorizationLocalState::ReceivedAuthProtocolResponse) => {
                match self.required_local_auth {
                    #[cfg(feature = "challenge-authorization")]
                    Some(ConnectionAuthorizationType::Challenge { .. }) => {
                        if protocol_request
                            .accepted_authorization_type
                            .iter()
                            .any(|t| matches!(t, PeerAuthorizationType::Challenge))
                        {
                            let nonce_request = AuthorizationMessage::AuthChallengeNonceRequest(
                                AuthChallengeNonceRequest,
                            );

                            let action = AuthorizationLocalAction::Challenge(
                                ChallengeAuthorizationLocalAction::SendAuthChallengeNonceRequest,
                            );
                            if self
                                .auth_manager
                                .next_local_state(context.source_connection_id(), action)
                                .is_err()
                            {
                                error!(
                                    "Unable to transition from ReceivedAuthProtocolResponse to \
                                    WaitingForAuthChallengeNonceResponse"
                                )
                            };

                            msg_bytes = IntoBytes::<network::NetworkMessage>::into_bytes(
                                NetworkMessage::from(nonce_request),
                            )?;
                        } else {
                            send_authorization_error(
                                &self.auth_manager,
                                context.source_id(),
                                context.source_connection_id(),
                                sender,
                                "Required authorization type not supported",
                            )?;

                            return Ok(());
                        }
                    }
                    #[cfg(feature = "trust-authorization")]
                    Some(ConnectionAuthorizationType::Trust { .. }) => {
                        if protocol_request
                            .accepted_authorization_type
                            .iter()
                            .any(|t| matches!(t, PeerAuthorizationType::Trust))
                        {
                            let trust_request =
                                AuthorizationMessage::AuthTrustRequest(AuthTrustRequest {
                                    identity: self.identity.clone(),
                                });

                            if self
                                .auth_manager
                                .next_local_state(
                                    context.source_connection_id(),
                                    AuthorizationLocalAction::Trust(
                                        TrustAuthorizationLocalAction::SendAuthTrustRequest(
                                            Identity::Trust {
                                                identity: self.identity.to_string(),
                                            },
                                        ),
                                    ),
                                )
                                .is_err()
                            {
                                error!(
                                    "Unable to transition from ReceivedAuthProtocolResponse to \
                                    WaitingForAuthTrustResponse"
                                )
                            };

                            msg_bytes = IntoBytes::<network::NetworkMessage>::into_bytes(
                                NetworkMessage::from(trust_request),
                            )?;
                        } else {
                            send_authorization_error(
                                &self.auth_manager,
                                context.source_id(),
                                context.source_connection_id(),
                                sender,
                                "Required authorization type not supported",
                            )?;

                            return Ok(());
                        }
                    }
                    _ => {
                        #[cfg(feature = "trust-authorization")]
                        if protocol_request
                            .accepted_authorization_type
                            .iter()
                            .any(|t| matches!(t, PeerAuthorizationType::Trust))
                        {
                            let trust_request =
                                AuthorizationMessage::AuthTrustRequest(AuthTrustRequest {
                                    identity: self.identity.clone(),
                                });

                            if self
                                .auth_manager
                                .next_local_state(
                                    context.source_connection_id(),
                                    AuthorizationLocalAction::Trust(
                                        TrustAuthorizationLocalAction::SendAuthTrustRequest(
                                            Identity::Trust {
                                                identity: self.identity.to_string(),
                                            },
                                        ),
                                    ),
                                )
                                .is_err()
                            {
                                error!(
                                    "Unable to transition from ReceivedAuthProtocolResponse to \
                                    WaitingForAuthTrustResponse"
                                )
                            };

                            msg_bytes = IntoBytes::<network::NetworkMessage>::into_bytes(
                                NetworkMessage::from(trust_request),
                            )?;
                        }

                        #[cfg(feature = "challenge-authorization")]
                        if protocol_request
                            .accepted_authorization_type
                            .iter()
                            .any(|t| matches!(t, PeerAuthorizationType::Challenge))
                        {
                            let nonce_request = AuthorizationMessage::AuthChallengeNonceRequest(
                                AuthChallengeNonceRequest,
                            );

                            let action = AuthorizationLocalAction::Challenge(
                                ChallengeAuthorizationLocalAction::SendAuthChallengeNonceRequest,
                            );
                            if self
                                .auth_manager
                                .next_local_state(context.source_connection_id(), action)
                                .is_err()
                            {
                                error!(
                                    "Unable to transition from ReceivedAuthProtocolResponse to \
                                    WaitingForAuthChallengeNonceResponse"
                                )
                            };

                            msg_bytes = IntoBytes::<network::NetworkMessage>::into_bytes(
                                NetworkMessage::from(nonce_request),
                            )?;
                        }

                        #[cfg(not(any(
                            feature = "trust-authorization",
                            feature = "challenge-authorization"
                        )))]
                        {
                            send_authorization_error(
                                &self.auth_manager,
                                context.source_id(),
                                context.source_connection_id(),
                                sender,
                                "Required authorization type not supported",
                            )?;

                            return Ok(());
                        }
                    }
                };

                sender
                    .send(context.source_id().clone(), msg_bytes)
                    .map_err(|(recipient, payload)| {
                        DispatchError::NetworkSendError((recipient.into(), payload))
                    })?;
            }
            Ok(next_state) => panic!("Should not have been able to transition to {}", next_state),
        }
        Ok(())
    }
}

/// Handler for the Authorization Complete Message Type
pub struct AuthCompleteHandler {
    auth_manager: AuthorizationManagerStateMachine,
}

impl AuthCompleteHandler {
    pub fn new(auth_manager: AuthorizationManagerStateMachine) -> Self {
        Self { auth_manager }
    }
}

impl Handler for AuthCompleteHandler {
    type Source = ConnectionId;
    type MessageType = authorization::AuthorizationMessageType;
    type Message = authorization::AuthComplete;

    fn match_type(&self) -> Self::MessageType {
        authorization::AuthorizationMessageType::AUTH_COMPLETE
    }

    fn handle(
        &self,
        _msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        debug!(
            "Received authorization complete from {}",
            context.source_connection_id()
        );

        match self
            .auth_manager
            .received_complete(context.source_connection_id())
        {
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
            Ok(()) => (),
        }

        Ok(())
    }
}

fn send_authorization_error(
    auth_manager: &AuthorizationManagerStateMachine,
    source_id: &str,
    connection_id: &str,
    sender: &dyn MessageSender<ConnectionId>,
    error_string: &str,
) -> Result<(), DispatchError> {
    let response = AuthorizationMessage::AuthorizationError(
        AuthorizationError::AuthorizationRejected(error_string.into()),
    );

    let msg_bytes =
        IntoBytes::<network::NetworkMessage>::into_bytes(NetworkMessage::from(response))?;

    sender
        .send(source_id.into(), msg_bytes)
        .map_err(|(recipient, payload)| {
            DispatchError::NetworkSendError((recipient.into(), payload))
        })?;

    if auth_manager
        .next_remote_state(connection_id, AuthorizationRemoteAction::Unauthorizing)
        .is_err()
    {
        warn!(
            "Unable to update state to Unauthorizing for {}",
            connection_id,
        )
    };

    Ok(())
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

    use crate::network::auth::{AuthorizationDispatchBuilder, Identity, ManagedAuthorizationState};
    use crate::protocol::authorization::AuthComplete;
    use crate::protos::network::NetworkMessageType;
    use crate::protos::{authorization, network};
    #[cfg(feature = "challenge-authorization")]
    use crate::public_key;

    /// Test that an auth protocol request is properly handled via the dispatcher when no
    /// required authorization types are set
    ///
    /// This is verified by:
    ///
    /// 1) no error from the dispatcher
    /// 2) the handler should send an AuthProtocolResponse with both trust and challenge
    #[test]
    #[cfg(feature = "trust-authorization")]
    fn protocol_request_dispatch_no_required_auth() {
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
                .with_signers(&[new_signer()])
                .with_nonce(&vec![])
                .with_expected_authorization(None)
                .with_local_authorization(None)
                .with_verifier(Box::new(NoopVerifier))
        }

        let dispatcher = dispatcher_builder
            .build(dispatch_sender, auth_mgr)
            .expect("Unable to build authorization dispatcher");

        let connection_id = "test_connection".to_string();
        let msg_bytes = IntoBytes::<authorization::AuthorizationMessage>::into_bytes(
            AuthorizationMessage::AuthProtocolRequest(AuthProtocolRequest {
                auth_protocol_min: 1,
                auth_protocol_max: 1,
            }),
        )
        .expect("Unable to get message bytes for auth protocol request");

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

        let auth_protocol_response: authorization::AuthProtocolResponse = expect_auth_message(
            authorization::AuthorizationMessageType::AUTH_PROTOCOL_RESPONSE,
            &message_bytes,
        );
        assert_eq!(1, auth_protocol_response.get_auth_protocol());

        assert_eq!(
            vec![
                authorization::AuthProtocolResponse_PeerAuthorizationType::TRUST,
                #[cfg(feature = "challenge-authorization")]
                authorization::AuthProtocolResponse_PeerAuthorizationType::CHALLENGE
            ],
            auth_protocol_response
                .get_accepted_authorization_type()
                .to_vec()
        );
    }

    /// Test that an auth protocol request is properly handled via the dispatcher when challenge
    /// is set as required authorization types
    ///
    /// This is verified by:
    ///
    /// 1) no error from the dispatcher
    /// 2) the handler should send an AuthProtocolResponse with only challenge
    #[test]
    #[cfg(feature = "challenge-authorization")]
    fn protocol_request_dispatch_challenge_required_auth() {
        let auth_mgr = AuthorizationManagerStateMachine::default();
        let mock_sender = MockSender::new();
        let dispatch_sender = mock_sender.clone();
        let local_signer = new_signer();
        let nonce: Vec<u8> = (0..70).map(|_| rand::random::<u8>()).collect();
        let dispatcher = AuthorizationDispatchBuilder::new()
            .with_identity("mock_identity")
            .with_signers(&[local_signer.clone()])
            .with_nonce(&nonce)
            .with_expected_authorization(Some(ConnectionAuthorizationType::Challenge {
                public_key: public_key::PublicKey::from_bytes(
                    new_signer()
                        .public_key()
                        .expect("unable to get public key")
                        .into_bytes(),
                ),
            }))
            .with_local_authorization(Some(ConnectionAuthorizationType::Challenge {
                public_key: public_key::PublicKey::from_bytes(
                    local_signer
                        .public_key()
                        .expect("unable to get public key")
                        .into_bytes(),
                ),
            }))
            .with_verifier(Box::new(NoopVerifier))
            .build(dispatch_sender, auth_mgr)
            .expect("Unable to build authorization dispatcher");

        let connection_id = "test_connection".to_string();
        let msg_bytes = IntoBytes::<authorization::AuthorizationMessage>::into_bytes(
            AuthorizationMessage::AuthProtocolRequest(AuthProtocolRequest {
                auth_protocol_min: 1,
                auth_protocol_max: 1,
            }),
        )
        .expect("Unable to get message bytes");

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

        let auth_protocol_response: authorization::AuthProtocolResponse = expect_auth_message(
            authorization::AuthorizationMessageType::AUTH_PROTOCOL_RESPONSE,
            &message_bytes,
        );
        assert_eq!(1, auth_protocol_response.get_auth_protocol());

        assert_eq!(
            vec![authorization::AuthProtocolResponse_PeerAuthorizationType::CHALLENGE],
            auth_protocol_response
                .get_accepted_authorization_type()
                .to_vec()
        );
    }

    /// Test that an AuthComplete is properly handled. Also verify state is set to
    /// AuthorizedAndComplete
    ///
    /// This is verified by:
    ///
    /// 1) no error from the dispatcher
    /// 2) the handler should send a AuthComplete
    /// 3) verify state is AuthorizedAndComplete and Done(Identity)
    #[test]
    #[cfg(feature = "trust-authorization")]
    fn auth_complete() {
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
                    local_state: AuthorizationLocalState::WaitForComplete,
                    remote_state: AuthorizationRemoteState::Done(Identity::Trust {
                        identity: "other_identity".to_string(),
                    }),
                    received_complete: false,
                    local_authorization: None,
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
            AuthorizationMessage::AuthComplete(AuthComplete),
        )
        .expect("Unable to get message bytes");

        assert!(dispatcher
            .dispatch(
                connection_id.clone().into(),
                &NetworkMessageType::AUTHORIZATION,
                msg_bytes
            )
            .is_ok());

        assert_eq!(mock_sender.next_outbound(), None);

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

    #[cfg(feature = "challenge-authorization")]
    fn new_signer() -> Box<dyn Signer> {
        let context = Secp256k1Context::new();
        let key = context.new_random_private_key();
        context.new_signer(key)
    }
}
