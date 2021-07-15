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

#[cfg(feature = "challenge-authorization")]
use cylinder::{PublicKey, Signature, Signer, Verifier};

#[cfg(feature = "challenge-authorization")]
use crate::network::auth::state_machine::challenge_v1::{
    ChallengeAuthorizationLocalAction, ChallengeAuthorizationLocalState,
    ChallengeAuthorizationRemoteAction, ChallengeAuthorizationRemoteState,
};
#[cfg(feature = "trust-authorization")]
use crate::network::auth::state_machine::trust_v1::{
    TrustAuthorizationLocalAction, TrustAuthorizationRemoteAction, TrustAuthorizationRemoteState,
};
use crate::network::auth::{
    AuthorizationLocalAction, AuthorizationLocalState, AuthorizationManagerStateMachine,
    AuthorizationMessage, AuthorizationRemoteAction, AuthorizationRemoteState,
    ConnectionAuthorizationType, Identity,
};
use crate::network::dispatch::{
    ConnectionId, DispatchError, Handler, MessageContext, MessageSender,
};
#[cfg(feature = "challenge-authorization")]
use crate::protocol::authorization::{
    AuthChallengeNonceRequest, AuthChallengeNonceResponse, AuthChallengeSubmitRequest,
    AuthChallengeSubmitResponse, SubmitRequest,
};
use crate::protocol::authorization::{
    AuthComplete, AuthProtocolRequest, AuthProtocolResponse, AuthorizationError,
    PeerAuthorizationType,
};
#[cfg(feature = "trust-authorization")]
use crate::protocol::authorization::{AuthTrustRequest, AuthTrustResponse};
use crate::protocol::network::NetworkMessage;
use crate::protocol::{PEER_AUTHORIZATION_PROTOCOL_MIN, PEER_AUTHORIZATION_PROTOCOL_VERSION};
use crate::protos::authorization;
use crate::protos::network;
use crate::protos::prelude::*;
#[cfg(feature = "challenge-authorization")]
use crate::public_key;

/// Handler for the Authorization Protocol Request Message Type
pub struct AuthProtocolRequestHandler {
    auth_manager: AuthorizationManagerStateMachine,
    #[cfg(feature = "challenge-authorization")]
    challenge_configured: bool,
    #[cfg(feature = "challenge-authorization")]
    expected_authorization: Option<ConnectionAuthorizationType>,
}

impl AuthProtocolRequestHandler {
    pub fn new(
        auth_manager: AuthorizationManagerStateMachine,
        #[cfg(feature = "challenge-authorization")] challenge_configured: bool,
        #[cfg(feature = "challenge-authorization")] expected_authorization: Option<
            ConnectionAuthorizationType,
        >,
    ) -> Self {
        Self {
            auth_manager,
            #[cfg(feature = "challenge-authorization")]
            challenge_configured,
            #[cfg(feature = "challenge-authorization")]
            expected_authorization,
        }
    }
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
                    #[cfg(feature = "challenge-authorization")]
                    Some(ConnectionAuthorizationType::Challenge { .. }) => {
                        accepted_authorization_type = vec![PeerAuthorizationType::Challenge]
                    }
                    _ => {
                        // if trust is enabled it was already added
                        #[cfg(feature = "challenge-authorization")]
                        if self.challenge_configured {
                            accepted_authorization_type.push(PeerAuthorizationType::Challenge)
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

impl AuthProtocolResponseHandler {
    pub fn new(
        auth_manager: AuthorizationManagerStateMachine,
        #[cfg(feature = "trust-authorization")] identity: String,
        required_local_auth: Option<ConnectionAuthorizationType>,
    ) -> Self {
        Self {
            auth_manager,
            #[cfg(feature = "trust-authorization")]
            identity,
            required_local_auth,
        }
    }
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
                                        TrustAuthorizationLocalAction::SendAuthTrustRequest,
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
                                        TrustAuthorizationLocalAction::SendAuthTrustRequest,
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

/// Handler for the Authorization Trust Request Message Type
#[cfg(feature = "trust-authorization")]
pub struct AuthTrustRequestHandler {
    auth_manager: AuthorizationManagerStateMachine,
}

#[cfg(feature = "trust-authorization")]
impl AuthTrustRequestHandler {
    pub fn new(auth_manager: AuthorizationManagerStateMachine) -> Self {
        Self { auth_manager }
    }
}

#[cfg(feature = "trust-authorization")]
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

#[cfg(feature = "trust-authorization")]
/// Handler for the Authorization Trust Response Message Type
pub struct AuthTrustResponseHandler {
    auth_manager: AuthorizationManagerStateMachine,
}

#[cfg(feature = "trust-authorization")]
impl AuthTrustResponseHandler {
    pub fn new(auth_manager: AuthorizationManagerStateMachine) -> Self {
        Self { auth_manager }
    }
}

#[cfg(feature = "trust-authorization")]
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

/// Handler for the Authorization Challenge Nonce Request Message Type
#[cfg(feature = "challenge-authorization")]
pub struct AuthChallengeNonceRequestHandler {
    auth_manager: AuthorizationManagerStateMachine,
    nonce: Vec<u8>,
}

#[cfg(feature = "challenge-authorization")]
impl AuthChallengeNonceRequestHandler {
    pub fn new(auth_manager: AuthorizationManagerStateMachine, nonce: Vec<u8>) -> Self {
        Self {
            auth_manager,
            nonce,
        }
    }
}

#[cfg(feature = "challenge-authorization")]
impl Handler for AuthChallengeNonceRequestHandler {
    type Source = ConnectionId;
    type MessageType = authorization::AuthorizationMessageType;
    type Message = authorization::AuthChallengeNonceRequest;

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

        match self.auth_manager.next_remote_state(
            context.source_connection_id(),
            AuthorizationRemoteAction::Challenge(
                ChallengeAuthorizationRemoteAction::ReceiveAuthChallengeNonceRequest,
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
            Ok(AuthorizationRemoteState::Challenge(
                ChallengeAuthorizationRemoteState::ReceivedAuthChallengeNonce,
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
                    .next_remote_state(
                        context.source_connection_id(),
                        AuthorizationRemoteAction::Challenge(
                            ChallengeAuthorizationRemoteAction::SendAuthChallengeNonceResponse,
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
            Ok(next_state) => panic!("Should not have been able to transition to {}", next_state),
        }

        Ok(())
    }
}

/// Handler for the Authorization Challenge Nonce Response Message Type
#[cfg(feature = "challenge-authorization")]
pub struct AuthChallengeNonceResponseHandler {
    auth_manager: AuthorizationManagerStateMachine,
    signers: Vec<Box<dyn Signer>>,
}

#[cfg(feature = "challenge-authorization")]
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

#[cfg(feature = "challenge-authorization")]
impl Handler for AuthChallengeNonceResponseHandler {
    type Source = ConnectionId;
    type MessageType = authorization::AuthorizationMessageType;
    type Message = authorization::AuthChallengeNonceResponse;

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

        let nonce_request = AuthChallengeNonceResponse::from_proto(msg)?;

        let mut public_keys = vec![];

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

                let public_key = signer
                    .public_key()
                    .map_err(|err| {
                        DispatchError::HandleError(format!(
                            "Unable to get public key for signer: {}",
                            err
                        ))
                    })?
                    .into_bytes();

                public_keys.push(public_key.clone());

                Ok(SubmitRequest {
                    public_key,
                    signature,
                })
            })
            .collect::<Result<Vec<SubmitRequest>, DispatchError>>()?;

        match self.auth_manager.next_local_state(
            context.source_connection_id(),
            AuthorizationLocalAction::Challenge(
                ChallengeAuthorizationLocalAction::ReceiveAuthChallengeNonceResponse,
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
            Ok(AuthorizationLocalState::Challenge(
                ChallengeAuthorizationLocalState::ReceivedAuthChallengeNonceResponse,
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
                    .next_local_state(
                        context.source_connection_id(),
                        AuthorizationLocalAction::Challenge(
                            ChallengeAuthorizationLocalAction::SendAuthChallengeSubmitRequest,
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
            Ok(next_state) => panic!("Should not have been able to transition to {}", next_state),
        }

        Ok(())
    }
}

/// Handler for the Authorization Challenge Submit Request Message Type
#[cfg(feature = "challenge-authorization")]
pub struct AuthChallengeSubmitRequestHandler {
    auth_manager: AuthorizationManagerStateMachine,
    verifer: Box<dyn Verifier>,
    nonce: Vec<u8>,
    expected_public_key: Option<public_key::PublicKey>,
}

#[cfg(feature = "challenge-authorization")]
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

#[cfg(feature = "challenge-authorization")]
impl Handler for AuthChallengeSubmitRequestHandler {
    type Source = ConnectionId;
    type MessageType = authorization::AuthorizationMessageType;
    type Message = authorization::AuthChallengeSubmitRequest;

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

        let submit_msg = AuthChallengeSubmitRequest::from_proto(msg)?;
        let mut public_keys = vec![];

        for request in submit_msg.submit_requests {
            let verified = self
                .verifer
                .verify(
                    &self.nonce,
                    &Signature::new(request.signature.to_vec()),
                    &PublicKey::new(request.public_key.to_vec()),
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
            public_keys.push(request.public_key.to_vec());
        }

        let identity = if let Some(public_key) = &self.expected_public_key {
            if public_keys.contains(&public_key.as_slice().to_vec()) {
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
        } else if public_keys.len() == 1 {
            // we know this is safe because of above length check
            // defaults to the first key in the list
            public_key::PublicKey::from_bytes(public_keys[0].clone())
        } else {
            let error_string = {
                if public_keys.is_empty() {
                    "No public keys submitted".to_string()
                } else {
                    "Too many public keys submitted".to_string()
                }
            };

            send_authorization_error(
                &self.auth_manager,
                context.source_id(),
                context.source_connection_id(),
                sender,
                &error_string,
            )?;

            return Ok(());
        };

        match self.auth_manager.next_remote_state(
            context.source_connection_id(),
            AuthorizationRemoteAction::Challenge(
                ChallengeAuthorizationRemoteAction::ReceiveAuthChallengeSubmitRequest(
                    Identity::Challenge {
                        public_key: identity,
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
            Ok(AuthorizationRemoteState::Challenge(
                ChallengeAuthorizationRemoteState::ReceivedAuthChallengeSubmitRequest(_),
            )) => {
                let auth_msg =
                    AuthorizationMessage::AuthChallengeSubmitResponse(AuthChallengeSubmitResponse);

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
                AuthorizationRemoteAction::Challenge(
                    ChallengeAuthorizationRemoteAction::SendAuthChallengeSubmitResponse,
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
#[cfg(feature = "challenge-authorization")]
pub struct AuthChallengeSubmitResponseHandler {
    auth_manager: AuthorizationManagerStateMachine,
}

#[cfg(feature = "challenge-authorization")]
impl AuthChallengeSubmitResponseHandler {
    pub fn new(auth_manager: AuthorizationManagerStateMachine) -> Self {
        Self { auth_manager }
    }
}

#[cfg(feature = "challenge-authorization")]
impl Handler for AuthChallengeSubmitResponseHandler {
    type Source = ConnectionId;
    type MessageType = authorization::AuthorizationMessageType;
    type Message = authorization::AuthChallengeSubmitResponse;

    fn match_type(&self) -> Self::MessageType {
        authorization::AuthorizationMessageType::AUTH_CHALLENGE_SUBMIT_RESPONSE
    }

    fn handle(
        &self,
        _msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        debug!(
            "Received authorization challenge submit response from {}",
            context.source_connection_id()
        );

        match self.auth_manager.next_local_state(
            context.source_connection_id(),
            AuthorizationLocalAction::Challenge(
                ChallengeAuthorizationLocalAction::ReceiveAuthChallengeSubmitResponse,
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
            Ok(AuthorizationLocalState::Authorized) => {
                let auth_msg = AuthorizationMessage::AuthComplete(AuthComplete);
                let msg_bytes = IntoBytes::<network::NetworkMessage>::into_bytes(
                    NetworkMessage::from(auth_msg),
                )?;
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
                    Ok(next_state) => {
                        panic!("Should not have been able to transition to {}", next_state)
                    }
                };
            }
            Ok(next_state) => panic!("Should not have been able to transition to {}", next_state),
        };
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

    use cylinder::{
        secp256k1::Secp256k1Context, Context, PublicKey, Signature, VerificationError, Verifier,
    };
    use protobuf::Message;

    #[cfg(feature = "challenge-authorization")]
    use crate::network::auth::state_machine::challenge_v1::ChallengeAuthorizationLocalState;
    #[cfg(feature = "trust-authorization")]
    use crate::network::auth::state_machine::trust_v1::TrustAuthorizationLocalState;
    use crate::network::auth::AuthorizationDispatchBuilder;
    use crate::network::auth::ManagedAuthorizationState;
    use crate::protos::network::NetworkMessageType;
    use crate::protos::{authorization, network};

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

    /// Test that an auth protocol response is properly handled via the dispatcher when trust is
    /// is set as the accepted authorization type.
    ///
    /// This is verified by:
    ///
    /// 1) no error from the dispatcher
    /// 2) the handler should send an AuthTrustRequest with the provided identity
    #[test]
    #[cfg(feature = "trust-authorization")]
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
    #[cfg(feature = "trust-authorization")]
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
    #[cfg(feature = "trust-authorization")]
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
    #[cfg(feature = "trust-authorization")]
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

    /// Test that a protocol response is properly handled when only challenge is in
    /// accepted_authorization_type
    ///
    /// This is verified by:
    ///
    /// 1) no error from the dispatcher
    /// 2) the handler should send AuthChallengeNonceRequest
    #[test]
    #[cfg(feature = "challenge-authorization")]
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
                    local_state: AuthorizationLocalState::WaitingForAuthProtocolResponse,
                    remote_state: AuthorizationRemoteState::SentAuthProtocolResponse,
                    received_complete: true,
                },
            );
        let mock_sender = MockSender::new();
        let dispatch_sender = mock_sender.clone();
        let local_signer = new_signer();
        let other_signer = new_signer();
        let nonce: Vec<u8> = (0..70).map(|_| rand::random::<u8>()).collect();
        let dispatcher = AuthorizationDispatchBuilder::new()
            .with_identity("mock_identity")
            .with_signers(&[local_signer.clone()])
            .with_nonce(&nonce)
            .with_expected_authorization(Some(ConnectionAuthorizationType::Challenge {
                public_key: public_key::PublicKey::from_bytes(
                    other_signer
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
            .build(dispatch_sender, auth_mgr.clone())
            .expect("Unable to build authorization dispatcher");

        let msg_bytes = IntoBytes::<authorization::AuthorizationMessage>::into_bytes(
            AuthorizationMessage::AuthProtocolResponse(AuthProtocolResponse {
                auth_protocol: 1,
                accepted_authorization_type: vec![PeerAuthorizationType::Challenge],
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
            managed_state.local_state,
            AuthorizationLocalState::Challenge(
                ChallengeAuthorizationLocalState::WaitingForAuthChallengeNonceResponse
            ),
        );
        assert_eq!(
            managed_state.remote_state,
            AuthorizationRemoteState::SentAuthProtocolResponse
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
    #[cfg(feature = "challenge-authorization")]
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
                    local_state: AuthorizationLocalState::Challenge(
                        ChallengeAuthorizationLocalState::WaitingForAuthChallengeNonceResponse,
                    ),
                    remote_state: AuthorizationRemoteState::SentAuthProtocolResponse,
                    received_complete: true,
                },
            );
        let mock_sender = MockSender::new();
        let dispatch_sender = mock_sender.clone();
        let local_signer = new_signer();
        let other_signer = new_signer();
        let nonce: Vec<u8> = (0..70).map(|_| rand::random::<u8>()).collect();
        let dispatcher = AuthorizationDispatchBuilder::new()
            .with_identity("mock_identity")
            .with_signers(&[local_signer.clone()])
            .with_nonce(&nonce)
            .with_expected_authorization(Some(ConnectionAuthorizationType::Challenge {
                public_key: public_key::PublicKey::from_bytes(
                    other_signer
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
            .build(dispatch_sender, auth_mgr.clone())
            .expect("Unable to build authorization dispatcher");

        let msg_bytes = IntoBytes::<authorization::AuthorizationMessage>::into_bytes(
            AuthorizationMessage::AuthChallengeNonceRequest(AuthChallengeNonceRequest),
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
            managed_state.local_state,
            AuthorizationLocalState::Challenge(
                ChallengeAuthorizationLocalState::WaitingForAuthChallengeNonceResponse
            ),
        );
        assert_eq!(
            managed_state.remote_state,
            AuthorizationRemoteState::Challenge(
                ChallengeAuthorizationRemoteState::WaitingForAuthChallengeSubmitRequest
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
    #[cfg(feature = "challenge-authorization")]
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
                    local_state: AuthorizationLocalState::Challenge(
                        ChallengeAuthorizationLocalState::WaitingForAuthChallengeNonceResponse,
                    ),
                    remote_state: AuthorizationRemoteState::Challenge(
                        ChallengeAuthorizationRemoteState::WaitingForAuthChallengeSubmitRequest,
                    ),
                    received_complete: true,
                },
            );
        let mock_sender = MockSender::new();
        let dispatch_sender = mock_sender.clone();
        let local_signer = new_signer();
        let other_signer = new_signer();
        let nonce: Vec<u8> = (0..70).map(|_| rand::random::<u8>()).collect();
        let dispatcher = AuthorizationDispatchBuilder::new()
            .with_identity("mock_identity")
            .with_signers(&[local_signer.clone()])
            .with_nonce(&nonce)
            .with_expected_authorization(Some(ConnectionAuthorizationType::Challenge {
                public_key: public_key::PublicKey::from_bytes(
                    other_signer
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
            .build(dispatch_sender, auth_mgr.clone())
            .expect("Unable to build authorization dispatcher");

        let msg_bytes = IntoBytes::<authorization::AuthorizationMessage>::into_bytes(
            AuthorizationMessage::AuthChallengeNonceResponse(AuthChallengeNonceResponse { nonce }),
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
            managed_state.local_state,
            AuthorizationLocalState::Challenge(
                ChallengeAuthorizationLocalState::WaitingForAuthChallengeSubmitResponse
            ),
        );
        assert_eq!(
            managed_state.remote_state,
            AuthorizationRemoteState::Challenge(
                ChallengeAuthorizationRemoteState::WaitingForAuthChallengeSubmitRequest
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
    #[cfg(feature = "challenge-authorization")]
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
                    local_state: AuthorizationLocalState::Challenge(
                        ChallengeAuthorizationLocalState::WaitingForAuthChallengeSubmitResponse,
                    ),
                    remote_state: AuthorizationRemoteState::Challenge(
                        ChallengeAuthorizationRemoteState::WaitingForAuthChallengeSubmitRequest,
                    ),
                    received_complete: false,
                },
            );
        let mock_sender = MockSender::new();
        let dispatch_sender = mock_sender.clone();
        let local_signer = new_signer();
        let other_signer = new_signer();
        let nonce: Vec<u8> = (0..70).map(|_| rand::random::<u8>()).collect();
        let dispatcher = AuthorizationDispatchBuilder::new()
            .with_identity("mock_identity")
            .with_signers(&[local_signer.clone()])
            .with_nonce(&nonce)
            .with_expected_authorization(Some(ConnectionAuthorizationType::Challenge {
                public_key: public_key::PublicKey::from_bytes(
                    other_signer
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
            .build(dispatch_sender, auth_mgr.clone())
            .expect("Unable to build authorization dispatcher");

        let msg_bytes = IntoBytes::<authorization::AuthorizationMessage>::into_bytes(
            AuthorizationMessage::AuthChallengeSubmitRequest(AuthChallengeSubmitRequest {
                submit_requests: vec![SubmitRequest {
                    public_key: other_signer
                        .public_key()
                        .expect("Unable to get public key")
                        .into_bytes(),
                    signature: other_signer
                        .sign(&nonce)
                        .expect("Unable to sign nonce")
                        .take_bytes(),
                }],
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
            managed_state.local_state,
            AuthorizationLocalState::Challenge(
                ChallengeAuthorizationLocalState::WaitingForAuthChallengeSubmitResponse
            ),
        );
        assert_eq!(
            managed_state.remote_state,
            AuthorizationRemoteState::Done(Identity::Challenge {
                public_key: public_key::PublicKey::from_bytes(
                    other_signer
                        .public_key()
                        .expect("Unable to get public key")
                        .into_bytes()
                ),
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
    #[cfg(feature = "challenge-authorization")]
    fn auth_challenge_submit_response() {
        let connection_id = "test_connection".to_string();
        let other_signer = new_signer();
        // need to setup expected authorization state
        let public_key = public_key::PublicKey::from_bytes(
            other_signer
                .public_key()
                .expect("unable to get public key")
                .into_bytes(),
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
                    local_state: AuthorizationLocalState::Challenge(
                        ChallengeAuthorizationLocalState::WaitingForAuthChallengeSubmitResponse,
                    ),
                    remote_state: AuthorizationRemoteState::Done(Identity::Challenge {
                        public_key: public_key.clone(),
                    }),
                    received_complete: false,
                },
            );
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
                    other_signer
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
            .build(dispatch_sender, auth_mgr.clone())
            .expect("Unable to build authorization dispatcher");

        let msg_bytes = IntoBytes::<authorization::AuthorizationMessage>::into_bytes(
            AuthorizationMessage::AuthChallengeSubmitResponse(AuthChallengeSubmitResponse),
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
            managed_state.local_state,
            AuthorizationLocalState::WaitForComplete
        );
        assert_eq!(
            managed_state.remote_state,
            AuthorizationRemoteState::Done(Identity::Challenge { public_key })
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
    #[cfg(feature = "challenge-authorization")]
    fn auth_challenge_submit_response_complete() {
        let connection_id = "test_connection".to_string();
        let other_signer = new_signer();
        // need to setup expected authorization state
        let public_key = public_key::PublicKey::from_bytes(
            other_signer
                .public_key()
                .expect("unable to get public key")
                .into_bytes(),
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
                    local_state: AuthorizationLocalState::Challenge(
                        ChallengeAuthorizationLocalState::WaitingForAuthChallengeSubmitResponse,
                    ),
                    remote_state: AuthorizationRemoteState::Done(Identity::Challenge {
                        public_key: public_key.clone(),
                    }),
                    received_complete: true,
                },
            );
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
                    other_signer
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
            .build(dispatch_sender, auth_mgr.clone())
            .expect("Unable to build authorization dispatcher");

        let msg_bytes = IntoBytes::<authorization::AuthorizationMessage>::into_bytes(
            AuthorizationMessage::AuthChallengeSubmitResponse(AuthChallengeSubmitResponse),
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
            managed_state.local_state,
            AuthorizationLocalState::AuthorizedAndComplete,
        );
        assert_eq!(
            managed_state.remote_state,
            AuthorizationRemoteState::Done(Identity::Challenge { public_key })
        );
        assert_eq!(managed_state.received_complete, true);
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

        assert_eq!(
            Ok(()),
            dispatcher.dispatch(
                connection_id.clone().into(),
                &NetworkMessageType::AUTHORIZATION,
                msg_bytes
            )
        );

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

    fn new_signer() -> Box<dyn Signer> {
        let context = Secp256k1Context::new();
        let key = context.new_random_private_key();
        context.new_signer(key)
    }
}
