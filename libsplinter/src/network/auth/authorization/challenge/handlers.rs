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

//! Message handlers for v1 authorization messages

use cylinder::{Signature, Signer, Verifier};

use crate::error::InternalError;
use crate::network::auth::state_machine::challenge_v1::{
    ChallengeAuthorizationAcceptingAction, ChallengeAuthorizationAcceptingState,
    ChallengeAuthorizationInitiatingAction, ChallengeAuthorizationInitiatingState,
};
use crate::network::auth::{
    AuthorizationAcceptingAction, AuthorizationAcceptingState, AuthorizationInitiatingAction,
    AuthorizationInitiatingState, AuthorizationManagerStateMachine, AuthorizationMessage, Identity,
};
use crate::network::dispatch::{
    ConnectionId, DispatchError, Handler, MessageContext, MessageSender, RawBytes,
};
use crate::protocol::authorization::AuthComplete;
use crate::protocol::authorization::{
    AuthChallengeNonceResponse, AuthChallengeSubmitRequest, AuthChallengeSubmitResponse,
    AuthorizationError, SubmitRequest,
};
use crate::protocol::network::NetworkMessage;
use crate::protos::authorization;
use crate::protos::network;
use crate::protos::prelude::*;
use crate::public_key;

/// Handler for the Authorization Challenge Nonce Request Message Type

pub struct AuthChallengeNonceRequestHandler {
    auth_manager: AuthorizationManagerStateMachine,
    nonce: Vec<u8>,
}

impl AuthChallengeNonceRequestHandler {
    pub fn new(auth_manager: AuthorizationManagerStateMachine, nonce: Vec<u8>) -> Self {
        Self {
            auth_manager,
            nonce,
        }
    }
}

impl Handler for AuthChallengeNonceRequestHandler {
    type Source = ConnectionId;
    type MessageType = authorization::AuthorizationMessageType;
    type Message = RawBytes;

    fn match_type(&self) -> Self::MessageType {
        authorization::AuthorizationMessageType::AUTH_CHALLENGE_NONCE_REQUEST
    }

    fn handle(
        &self,
        _msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        debug!(
            "Received authorization challenge nonce request from {}",
            context.source_connection_id()
        );

        match self.auth_manager.next_accepting_state(
            context.source_connection_id(),
            AuthorizationAcceptingAction::Challenge(
                ChallengeAuthorizationAcceptingAction::ReceiveAuthChallengeNonceRequest,
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
            Ok(AuthorizationAcceptingState::Challenge(
                ChallengeAuthorizationAcceptingState::ReceivedAuthChallengeNonce,
            )) => {
                let auth_msg =
                    AuthorizationMessage::AuthChallengeNonceResponse(AuthChallengeNonceResponse {
                        nonce: self.nonce.clone(),
                    });

                let msg_bytes = IntoBytes::<network::NetworkMessage>::into_bytes(
                    NetworkMessage::from(auth_msg),
                )?;

                sender
                    .send(context.source_id().clone(), msg_bytes)
                    .map_err(|(recipient, payload)| {
                        DispatchError::NetworkSendError((recipient.into(), payload))
                    })?;

                if self
                    .auth_manager
                    .next_accepting_state(
                        context.source_connection_id(),
                        AuthorizationAcceptingAction::Challenge(
                            ChallengeAuthorizationAcceptingAction::SendAuthChallengeNonceResponse,
                        ),
                    )
                    .is_err()
                {
                    error!(
                        "Unable to transition from ReceivedAuthChallengeNonceRequest to \
                        WaitingForAuthChallengeSubmitRequest"
                    );
                };
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

/// Handler for the Authorization Challenge Nonce Response Message Type

pub struct AuthChallengeNonceResponseHandler {
    auth_manager: AuthorizationManagerStateMachine,
    signers: Vec<Box<dyn Signer>>,
}

impl AuthChallengeNonceResponseHandler {
    pub fn new(
        auth_manager: AuthorizationManagerStateMachine,
        signers: Vec<Box<dyn Signer>>,
    ) -> Self {
        Self {
            auth_manager,
            signers,
        }
    }
}

impl Handler for AuthChallengeNonceResponseHandler {
    type Source = ConnectionId;
    type MessageType = authorization::AuthorizationMessageType;
    type Message = RawBytes;

    fn match_type(&self) -> Self::MessageType {
        authorization::AuthorizationMessageType::AUTH_CHALLENGE_NONCE_RESPONSE
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        debug!(
            "Received authorization challenge nonce response from {}",
            context.source_connection_id()
        );

        let nonce_request = AuthChallengeNonceResponse::from_bytes(msg.bytes())?;

        let submit_requests = self
            .signers
            .iter()
            .map(|signer| {
                let signature = signer
                    .sign(&nonce_request.nonce)
                    .map_err(|err| {
                        DispatchError::HandleError(format!(
                            "Unable to sign provided nonce: {}",
                            err
                        ))
                    })?
                    .take_bytes();

                let public_key = signer.public_key().map_err(|err| {
                    DispatchError::HandleError(format!(
                        "Unable to get public key for signer: {}",
                        err
                    ))
                })?;

                Ok(SubmitRequest {
                    public_key: public_key.into(),
                    signature,
                })
            })
            .collect::<Result<Vec<SubmitRequest>, DispatchError>>()?;

        match self.auth_manager.next_initiating_state(
            context.source_connection_id(),
            AuthorizationInitiatingAction::Challenge(
                ChallengeAuthorizationInitiatingAction::ReceiveAuthChallengeNonceResponse,
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
            Ok(AuthorizationInitiatingState::Challenge(
                ChallengeAuthorizationInitiatingState::ReceivedAuthChallengeNonceResponse,
            )) => {
                let auth_msg =
                    AuthorizationMessage::AuthChallengeSubmitRequest(AuthChallengeSubmitRequest {
                        submit_requests,
                    });

                let msg_bytes = IntoBytes::<network::NetworkMessage>::into_bytes(
                    NetworkMessage::from(auth_msg),
                )?;

                if self
                    .auth_manager
                    .next_initiating_state(
                        context.source_connection_id(),
                        AuthorizationInitiatingAction::Challenge(
                            ChallengeAuthorizationInitiatingAction::SendAuthChallengeSubmitRequest,
                        ),
                    )
                    .is_err()
                {
                    error!(
                        "Unable to transition from ReceivedAuthChallengeNonceResponse to \
                        WaitingForAuthChallengSubmitResponse"
                    )
                };

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

/// Handler for the Authorization Challenge Submit Request Message Type

pub struct AuthChallengeSubmitRequestHandler {
    auth_manager: AuthorizationManagerStateMachine,
    verifer: Box<dyn Verifier>,
    nonce: Vec<u8>,
    expected_public_key: Option<public_key::PublicKey>,
}

impl AuthChallengeSubmitRequestHandler {
    pub fn new(
        auth_manager: AuthorizationManagerStateMachine,
        verifer: Box<dyn Verifier>,
        nonce: Vec<u8>,
        expected_public_key: Option<public_key::PublicKey>,
    ) -> Self {
        Self {
            auth_manager,
            verifer,
            nonce,
            expected_public_key,
        }
    }
}

impl Handler for AuthChallengeSubmitRequestHandler {
    type Source = ConnectionId;
    type MessageType = authorization::AuthorizationMessageType;
    type Message = RawBytes;

    fn match_type(&self) -> Self::MessageType {
        authorization::AuthorizationMessageType::AUTH_CHALLENGE_SUBMIT_REQUEST
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        debug!(
            "Received authorization challenge submit request from {}",
            context.source_connection_id()
        );

        let submit_msg = AuthChallengeSubmitRequest::from_bytes(msg.bytes())?;
        let mut public_keys = vec![];

        for request in submit_msg.submit_requests {
            let verified = self
                .verifer
                .verify(
                    &self.nonce,
                    &Signature::new(request.signature.to_vec()),
                    &request.public_key.clone().into(),
                )
                .map_err(|err| {
                    DispatchError::HandleError(format!("Unable to verify submit request: {}", err))
                })?;
            if !verified {
                send_authorization_error(
                    &self.auth_manager,
                    context.source_id(),
                    context.source_connection_id(),
                    sender,
                    "Challenge signature was not valid",
                )?;

                return Ok(());
            }
            public_keys.push(request.public_key);
        }

        let identity = if let Some(public_key) = &self.expected_public_key {
            if public_keys.contains(public_key) {
                public_key.clone()
            } else {
                send_authorization_error(
                    &self.auth_manager,
                    context.source_id(),
                    context.source_connection_id(),
                    sender,
                    "Required public key not submitted",
                )?;

                return Ok(());
            }
        } else if !public_keys.is_empty() {
            // we know this is safe because of above length check
            // defaults to the first key in the list
            public_keys[0].clone()
        } else {
            send_authorization_error(
                &self.auth_manager,
                context.source_id(),
                context.source_connection_id(),
                sender,
                "No public keys submitted",
            )?;

            return Ok(());
        };

        match self.auth_manager.next_accepting_state(
            context.source_connection_id(),
            AuthorizationAcceptingAction::Challenge(
                ChallengeAuthorizationAcceptingAction::ReceiveAuthChallengeSubmitRequest(
                    Identity::Challenge {
                        public_key: identity.clone(),
                    },
                ),
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
            Ok(AuthorizationAcceptingState::Challenge(
                ChallengeAuthorizationAcceptingState::ReceivedAuthChallengeSubmitRequest(_),
            )) => {
                let auth_msg = AuthorizationMessage::AuthChallengeSubmitResponse(
                    AuthChallengeSubmitResponse {
                        public_key: identity,
                    },
                );

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

        if self
            .auth_manager
            .next_accepting_state(
                context.source_connection_id(),
                AuthorizationAcceptingAction::Challenge(
                    ChallengeAuthorizationAcceptingAction::SendAuthChallengeSubmitResponse,
                ),
            )
            .is_err()
        {
            error!("Unable to transition from ReceivedAuthChallengSubmitRequest to Done")
        };

        Ok(())
    }
}

/// Handler for the Authorization Challenge Submit Response Message Type

pub struct AuthChallengeSubmitResponseHandler {
    auth_manager: AuthorizationManagerStateMachine,
}

impl AuthChallengeSubmitResponseHandler {
    pub fn new(auth_manager: AuthorizationManagerStateMachine) -> Self {
        Self { auth_manager }
    }
}

impl Handler for AuthChallengeSubmitResponseHandler {
    type Source = ConnectionId;
    type MessageType = authorization::AuthorizationMessageType;
    type Message = RawBytes;

    fn match_type(&self) -> Self::MessageType {
        authorization::AuthorizationMessageType::AUTH_CHALLENGE_SUBMIT_RESPONSE
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        debug!(
            "Received authorization challenge submit response from {}",
            context.source_connection_id()
        );

        let submit_msg = AuthChallengeSubmitResponse::from_bytes(msg.bytes())?;

        let public_key = submit_msg.public_key;

        match self.auth_manager.next_initiating_state(
            context.source_connection_id(),
            AuthorizationInitiatingAction::Challenge(
                ChallengeAuthorizationInitiatingAction::ReceiveAuthChallengeSubmitResponse(
                    Identity::Challenge { public_key },
                ),
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
            Ok(AuthorizationInitiatingState::Authorized) => {
                let auth_msg = AuthorizationMessage::AuthComplete(AuthComplete);
                let msg_bytes = IntoBytes::<network::NetworkMessage>::into_bytes(
                    NetworkMessage::from(auth_msg),
                )?;
                sender
                    .send(context.source_id().clone(), msg_bytes)
                    .map_err(|(recipient, payload)| {
                        DispatchError::NetworkSendError((recipient.into(), payload))
                    })?;

                match self.auth_manager.next_initiating_state(
                    context.source_connection_id(),
                    AuthorizationInitiatingAction::SendAuthComplete,
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
                    Ok(AuthorizationInitiatingState::WaitForComplete) => (),
                    Ok(AuthorizationInitiatingState::AuthorizedAndComplete) => (),
                    Ok(next_state) => {
                        return Err(DispatchError::InternalError(InternalError::with_message(
                            format!("Should not have been able to transition to {}", next_state),
                        )))
                    }
                };
            }
            Ok(next_state) => {
                return Err(DispatchError::InternalError(InternalError::with_message(
                    format!("Should not have been able to transition to {}", next_state),
                )))
            }
        };
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
        .next_accepting_state(connection_id, AuthorizationAcceptingAction::Unauthorizing)
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

    use cylinder::{
        secp256k1::Secp256k1Context, Context, PublicKey, Signature, Signer, VerificationError,
        Verifier,
    };
    use protobuf::Message;

    use crate::network::auth::authorization::challenge::ChallengeAuthorization;
    use crate::network::auth::state_machine::challenge_v1::ChallengeAuthorizationInitiatingState;
    use crate::network::auth::{
        AuthorizationDispatchBuilder, ConnectionAuthorizationType, ManagedAuthorizationState,
    };
    use crate::protocol::authorization::{
        AuthChallengeNonceRequest, AuthProtocolResponse, PeerAuthorizationType,
    };
    use crate::protos::network::NetworkMessageType;
    use crate::protos::{authorization, network};

    /// Test that a protocol response is properly handled when only challenge is in
    /// accepted_authorization_type
    ///
    /// This is verified by:
    ///
    /// 1) no error from the dispatcher
    /// 2) the handler should send AuthChallengeNonceRequest
    #[test]
    fn protocol_response_challenge() {
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
                    initiating_state: AuthorizationInitiatingState::WaitingForAuthProtocolResponse,
                    accepting_state: AuthorizationAcceptingState::SentAuthProtocolResponse,
                    received_complete: true,
                    local_authorization: None,
                },
            );
        let mock_sender = MockSender::new();
        let dispatch_sender = mock_sender.clone();
        let local_signer = new_signer();
        let other_signer = new_signer();
        let nonce: Vec<u8> = (0..70).map(|_| rand::random::<u8>()).collect();
        let expected_authorization = Some(ConnectionAuthorizationType::Challenge {
            public_key: other_signer
                .public_key()
                .expect("unable to get public key")
                .into(),
        });
        let local_authorization = Some(ConnectionAuthorizationType::Challenge {
            public_key: local_signer
                .public_key()
                .expect("unable to get public key")
                .into(),
        });
        let dispatcher = AuthorizationDispatchBuilder::new()
            .with_identity("mock_identity")
            .add_authorization(Box::new(ChallengeAuthorization::new(
                vec![local_signer.clone()],
                nonce.clone(),
                Box::new(NoopVerifier),
                expected_authorization.clone(),
                local_authorization.clone(),
                auth_mgr.clone(),
            )))
            .with_expected_authorization(expected_authorization)
            .with_local_authorization(local_authorization)
            .build(dispatch_sender, auth_mgr.clone())
            .expect("Unable to build authorization dispatcher");

        let msg_bytes = IntoBytes::<authorization::AuthorizationMessage>::into_bytes(
            AuthorizationMessage::AuthProtocolResponse(AuthProtocolResponse {
                auth_protocol: 1,
                accepted_authorization_type: vec![PeerAuthorizationType::Challenge],
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

        let _nonce_request: authorization::AuthChallengeNonceRequest = expect_auth_message(
            authorization::AuthorizationMessageType::AUTH_CHALLENGE_NONCE_REQUEST,
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
            managed_state.initiating_state,
            AuthorizationInitiatingState::Challenge(
                ChallengeAuthorizationInitiatingState::WaitingForAuthChallengeNonceResponse
            ),
        );
        assert_eq!(
            managed_state.accepting_state,
            AuthorizationAcceptingState::SentAuthProtocolResponse
        );
        assert_eq!(managed_state.received_complete, true);
    }

    /// Test that an AuthChallengeNonceRequest is properly handled.
    ///
    /// This is verified by:
    ///
    /// 1) no error from the dispatcher
    /// 2) the handler should send AuthChallengeNonceResponse with the expected nonce
    #[test]

    fn auth_challenge_nonce_request() {
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
                    initiating_state: AuthorizationInitiatingState::Challenge(
                        ChallengeAuthorizationInitiatingState::WaitingForAuthChallengeNonceResponse,
                    ),
                    accepting_state: AuthorizationAcceptingState::SentAuthProtocolResponse,
                    received_complete: true,
                    local_authorization: None,
                },
            );
        let mock_sender = MockSender::new();
        let dispatch_sender = mock_sender.clone();
        let local_signer = new_signer();
        let other_signer = new_signer();
        let nonce: Vec<u8> = (0..70).map(|_| rand::random::<u8>()).collect();
        let expected_authorization = Some(ConnectionAuthorizationType::Challenge {
            public_key: other_signer
                .public_key()
                .expect("unable to get public key")
                .into(),
        });
        let local_authorization = Some(ConnectionAuthorizationType::Challenge {
            public_key: local_signer
                .public_key()
                .expect("unable to get public key")
                .into(),
        });
        let dispatcher = AuthorizationDispatchBuilder::new()
            .with_identity("mock_identity")
            .add_authorization(Box::new(ChallengeAuthorization::new(
                vec![local_signer.clone()],
                nonce.clone(),
                Box::new(NoopVerifier),
                expected_authorization.clone(),
                local_authorization.clone(),
                auth_mgr.clone(),
            )))
            .with_expected_authorization(expected_authorization)
            .with_local_authorization(local_authorization)
            .build(dispatch_sender, auth_mgr.clone())
            .expect("Unable to build authorization dispatcher");

        let msg_bytes = IntoBytes::<authorization::AuthorizationMessage>::into_bytes(
            AuthorizationMessage::AuthChallengeNonceRequest(AuthChallengeNonceRequest),
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

        let nonce_response: authorization::AuthChallengeNonceResponse = expect_auth_message(
            authorization::AuthorizationMessageType::AUTH_CHALLENGE_NONCE_RESPONSE,
            &message_bytes,
        );

        assert_eq!(&nonce, nonce_response.get_nonce());

        let managed_state = auth_mgr
            .shared
            .lock()
            .expect("lock poisoned")
            .states
            .get(&connection_id)
            .cloned()
            .expect("missing managed state for connection id");

        assert_eq!(
            managed_state.initiating_state,
            AuthorizationInitiatingState::Challenge(
                ChallengeAuthorizationInitiatingState::WaitingForAuthChallengeNonceResponse
            ),
        );
        assert_eq!(
            managed_state.accepting_state,
            AuthorizationAcceptingState::Challenge(
                ChallengeAuthorizationAcceptingState::WaitingForAuthChallengeSubmitRequest
            )
        );
        assert_eq!(managed_state.received_complete, true);
    }

    /// Test that an AuthChallengeNonceResponse is properly handled.
    ///
    /// This is verified by:
    ///
    /// 1) no error from the dispatcher
    /// 2) the handler should send a AuthChallengeSubmitRequest
    #[test]

    fn auth_challenge_nonce_response() {
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
                    initiating_state: AuthorizationInitiatingState::Challenge(
                        ChallengeAuthorizationInitiatingState::WaitingForAuthChallengeNonceResponse,
                    ),
                    accepting_state: AuthorizationAcceptingState::Challenge(
                        ChallengeAuthorizationAcceptingState::WaitingForAuthChallengeSubmitRequest,
                    ),
                    received_complete: true,
                    local_authorization: None,
                },
            );
        let mock_sender = MockSender::new();
        let dispatch_sender = mock_sender.clone();
        let local_signer = new_signer();
        let other_signer = new_signer();
        let nonce: Vec<u8> = (0..70).map(|_| rand::random::<u8>()).collect();
        let expected_authorization = Some(ConnectionAuthorizationType::Challenge {
            public_key: other_signer
                .public_key()
                .expect("unable to get public key")
                .into(),
        });
        let local_authorization = Some(ConnectionAuthorizationType::Challenge {
            public_key: local_signer
                .public_key()
                .expect("unable to get public key")
                .into(),
        });
        let dispatcher = AuthorizationDispatchBuilder::new()
            .with_identity("mock_identity")
            .add_authorization(Box::new(ChallengeAuthorization::new(
                vec![local_signer.clone()],
                nonce.clone(),
                Box::new(NoopVerifier),
                expected_authorization.clone(),
                local_authorization.clone(),
                auth_mgr.clone(),
            )))
            .with_expected_authorization(expected_authorization)
            .with_local_authorization(local_authorization)
            .build(dispatch_sender, auth_mgr.clone())
            .expect("Unable to build authorization dispatcher");

        let msg_bytes = IntoBytes::<authorization::AuthorizationMessage>::into_bytes(
            AuthorizationMessage::AuthChallengeNonceResponse(AuthChallengeNonceResponse { nonce }),
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

        let submit_requests: authorization::AuthChallengeSubmitRequest = expect_auth_message(
            authorization::AuthorizationMessageType::AUTH_CHALLENGE_SUBMIT_REQUEST,
            &message_bytes,
        );

        assert_eq!(1, submit_requests.get_submit_requests().len());

        let submit_request = submit_requests
            .get_submit_requests()
            .get(0)
            .expect("Unable to get submit request");

        assert_eq!(
            local_signer
                .public_key()
                .expect("unable to get public key")
                .as_slice(),
            submit_request.get_public_key()
        );

        assert!(!submit_request.get_signature().is_empty());

        let managed_state = auth_mgr
            .shared
            .lock()
            .expect("lock poisoned")
            .states
            .get(&connection_id)
            .cloned()
            .expect("missing managed state for connection id");

        assert_eq!(
            managed_state.initiating_state,
            AuthorizationInitiatingState::Challenge(
                ChallengeAuthorizationInitiatingState::WaitingForAuthChallengeSubmitResponse
            ),
        );
        assert_eq!(
            managed_state.accepting_state,
            AuthorizationAcceptingState::Challenge(
                ChallengeAuthorizationAcceptingState::WaitingForAuthChallengeSubmitRequest
            )
        );
        assert_eq!(managed_state.received_complete, true);
    }

    /// Test that an AuthChallengeSubmitRequest is properly handled.
    /// This is verified by:
    ///
    /// 1) no error from the dispatcher
    /// 2) the handler should send a AuthChallengeSubmitResponse
    #[test]
    fn auth_challenge_submit_request() {
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
                initiating_state: AuthorizationInitiatingState::Challenge(
                    ChallengeAuthorizationInitiatingState::WaitingForAuthChallengeSubmitResponse,
                ),
                accepting_state: AuthorizationAcceptingState::Challenge(
                    ChallengeAuthorizationAcceptingState::WaitingForAuthChallengeSubmitRequest,
                ),
                received_complete: false,
                local_authorization: None,
            },
        );
        let mock_sender = MockSender::new();
        let dispatch_sender = mock_sender.clone();
        let local_signer = new_signer();
        let other_signer = new_signer();
        let nonce: Vec<u8> = (0..70).map(|_| rand::random::<u8>()).collect();
        let expected_authorization = Some(ConnectionAuthorizationType::Challenge {
            public_key: other_signer
                .public_key()
                .expect("unable to get public key")
                .into(),
        });
        let local_authorization = Some(ConnectionAuthorizationType::Challenge {
            public_key: local_signer
                .public_key()
                .expect("unable to get public key")
                .into(),
        });
        let dispatcher = AuthorizationDispatchBuilder::new()
            .with_identity("mock_identity")
            .add_authorization(Box::new(ChallengeAuthorization::new(
                vec![local_signer.clone()],
                nonce.clone(),
                Box::new(NoopVerifier),
                expected_authorization.clone(),
                local_authorization.clone(),
                auth_mgr.clone(),
            )))
            .with_expected_authorization(expected_authorization)
            .with_local_authorization(local_authorization)
            .build(dispatch_sender, auth_mgr.clone())
            .expect("Unable to build authorization dispatcher");

        let msg_bytes = IntoBytes::<authorization::AuthorizationMessage>::into_bytes(
            AuthorizationMessage::AuthChallengeSubmitRequest(AuthChallengeSubmitRequest {
                submit_requests: vec![SubmitRequest {
                    public_key: other_signer
                        .public_key()
                        .expect("Unable to get public key")
                        .into(),
                    signature: other_signer
                        .sign(&nonce)
                        .expect("Unable to sign nonce")
                        .take_bytes(),
                }],
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

        let _submit_response: authorization::AuthChallengeSubmitResponse = expect_auth_message(
            authorization::AuthorizationMessageType::AUTH_CHALLENGE_SUBMIT_RESPONSE,
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
            managed_state.initiating_state,
            AuthorizationInitiatingState::Challenge(
                ChallengeAuthorizationInitiatingState::WaitingForAuthChallengeSubmitResponse
            ),
        );
        assert_eq!(
            managed_state.accepting_state,
            AuthorizationAcceptingState::Done(Identity::Challenge {
                public_key: other_signer
                    .public_key()
                    .expect("Unable to get public key")
                    .into()
            })
        );
        assert_eq!(managed_state.received_complete, false);
    }

    /// Test that an AuthChallengeSubmitResponse is properly handled. Also verify state is set to
    /// WaitForComplete because received_complete is false
    ///
    /// This is verified by:
    ///
    /// 1) no error from the dispatcher
    /// 2) the handler should send a AuthComplete
    /// 3) verify state is WaitForComplete and Done(Identity)
    #[test]
    fn auth_challenge_submit_response() {
        let connection_id = "test_connection".to_string();
        let other_signer = new_signer();
        // need to setup expected authorization state
        let public_key = public_key::PublicKey::from(
            other_signer.public_key().expect("unable to get public key"),
        );
        let auth_mgr = AuthorizationManagerStateMachine::default();
        auth_mgr
            .shared
            .lock()
            .expect("lock poisoned")
            .states
            .insert(
            connection_id.to_string(),
            ManagedAuthorizationState {
                initiating_state: AuthorizationInitiatingState::Challenge(
                    ChallengeAuthorizationInitiatingState::WaitingForAuthChallengeSubmitResponse,
                ),
                accepting_state: AuthorizationAcceptingState::Done(Identity::Challenge {
                    public_key: public_key.clone(),
                }),
                received_complete: false,
                local_authorization: None,
            },
        );
        let mock_sender = MockSender::new();
        let dispatch_sender = mock_sender.clone();
        let local_signer = new_signer();
        let nonce: Vec<u8> = (0..70).map(|_| rand::random::<u8>()).collect();
        let expected_authorization = Some(ConnectionAuthorizationType::Challenge {
            public_key: other_signer
                .public_key()
                .expect("unable to get public key")
                .into(),
        });
        let local_authorization = Some(ConnectionAuthorizationType::Challenge {
            public_key: local_signer
                .public_key()
                .expect("unable to get public key")
                .into(),
        });
        let dispatcher = AuthorizationDispatchBuilder::new()
            .with_identity("mock_identity")
            .add_authorization(Box::new(ChallengeAuthorization::new(
                vec![local_signer.clone()],
                nonce,
                Box::new(NoopVerifier),
                expected_authorization.clone(),
                local_authorization.clone(),
                auth_mgr.clone(),
            )))
            .with_expected_authorization(expected_authorization)
            .with_local_authorization(local_authorization)
            .build(dispatch_sender, auth_mgr.clone())
            .expect("Unable to build authorization dispatcher");

        let msg_bytes = IntoBytes::<authorization::AuthorizationMessage>::into_bytes(
            AuthorizationMessage::AuthChallengeSubmitResponse(AuthChallengeSubmitResponse {
                public_key: local_signer
                    .public_key()
                    .expect("unable to get public key")
                    .into(),
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

        let _auth_complete: authorization::AuthComplete = expect_auth_message(
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
            managed_state.initiating_state,
            AuthorizationInitiatingState::WaitForComplete
        );
        assert_eq!(
            managed_state.accepting_state,
            AuthorizationAcceptingState::Done(Identity::Challenge { public_key })
        );
        assert_eq!(managed_state.received_complete, false);
    }

    /// Test that an AuthChallengeSubmitResponse is properly handled. Also verify state is set to
    /// AuthorizedAndComplete because received_complete is true
    ///
    /// This is verified by:
    ///
    /// 1) no error from the dispatcher
    /// 2) the handler should send a AuthComplete
    /// 3) verify state is AuthorizedAndComplete and Done(Identity)
    #[test]
    fn auth_challenge_submit_response_complete() {
        let connection_id = "test_connection".to_string();
        let other_signer = new_signer();
        // need to setup expected authorization state
        let public_key = public_key::PublicKey::from(
            other_signer.public_key().expect("unable to get public key"),
        );
        let auth_mgr = AuthorizationManagerStateMachine::default();
        auth_mgr
            .shared
            .lock()
            .expect("lock poisoned")
            .states
            .insert(
            connection_id.to_string(),
            ManagedAuthorizationState {
                initiating_state: AuthorizationInitiatingState::Challenge(
                    ChallengeAuthorizationInitiatingState::WaitingForAuthChallengeSubmitResponse,
                ),
                accepting_state: AuthorizationAcceptingState::Done(Identity::Challenge {
                    public_key: public_key.clone(),
                }),
                received_complete: true,
                local_authorization: None,
            },
        );
        let mock_sender = MockSender::new();
        let dispatch_sender = mock_sender.clone();
        let local_signer = new_signer();
        let nonce: Vec<u8> = (0..70).map(|_| rand::random::<u8>()).collect();
        let expected_authorization = Some(ConnectionAuthorizationType::Challenge {
            public_key: other_signer
                .public_key()
                .expect("unable to get public key")
                .into(),
        });
        let local_authorization = Some(ConnectionAuthorizationType::Challenge {
            public_key: local_signer
                .public_key()
                .expect("unable to get public key")
                .into(),
        });
        let dispatcher = AuthorizationDispatchBuilder::new()
            .add_authorization(Box::new(ChallengeAuthorization::new(
                vec![local_signer.clone()],
                nonce,
                Box::new(NoopVerifier),
                expected_authorization.clone(),
                local_authorization.clone(),
                auth_mgr.clone(),
            )))
            .with_identity("mock_identity")
            .with_expected_authorization(expected_authorization)
            .with_local_authorization(local_authorization)
            .build(dispatch_sender, auth_mgr.clone())
            .expect("Unable to build authorization dispatcher");

        let msg_bytes = IntoBytes::<authorization::AuthorizationMessage>::into_bytes(
            AuthorizationMessage::AuthChallengeSubmitResponse(AuthChallengeSubmitResponse {
                public_key: local_signer
                    .public_key()
                    .expect("unable to get public key")
                    .into(),
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

        let _auth_complete: authorization::AuthComplete = expect_auth_message(
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
            managed_state.initiating_state,
            AuthorizationInitiatingState::AuthorizedAndComplete,
        );
        assert_eq!(
            managed_state.accepting_state,
            AuthorizationAcceptingState::Done(Identity::Challenge { public_key })
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
