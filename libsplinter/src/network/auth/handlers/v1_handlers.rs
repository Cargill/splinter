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
                warn!(
                    "Ignoring authorization protocol request from {}: {}",
                    context.source_connection_id(),
                    err
                );
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
                warn!(
                    "Ignoring authorization protocol request from {}: {}",
                    context.source_connection_id(),
                    err
                );
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
                warn!(
                    "Ignoring trust request message from connection {}: {}",
                    context.source_connection_id(),
                    err
                );
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
                warn!(
                    "Ignoring trust response message from connection {}: {}",
                    context.source_connection_id(),
                    err
                );
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
                warn!(
                    "Cannot transition connection from Authorized {}: {}",
                    context.source_connection_id(),
                    err
                );
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
                warn!(
                    "Ignoring challenge nonce request message from connection {}: {}",
                    context.source_connection_id(),
                    err
                );
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
                warn!(
                    "Ignoring challenge nonce response message from connection {}: {}",
                    context.source_connection_id(),
                    err
                );
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
    expected_public_key: Option<Vec<u8>>,
}

#[cfg(feature = "challenge-authorization")]
impl AuthChallengeSubmitRequestHandler {
    pub fn new(
        auth_manager: AuthorizationManagerStateMachine,
        verifer: Box<dyn Verifier>,
        nonce: Vec<u8>,
        expected_public_key: Option<Vec<u8>>,
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
            if public_keys.contains(&public_key) {
                public_key
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
            &public_keys[0]
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
                        public_key: identity.clone(),
                    },
                ),
            ),
        ) {
            Err(err) => {
                warn!(
                    "Ignoring challenge nonce response message from connection {}: {}",
                    context.source_connection_id(),
                    err
                );
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
                warn!(
                    "Ignoring challenge submit response message from connection {}: {}",
                    context.source_connection_id(),
                    err
                );
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
                        warn!(
                            "Cannot transition connection from Authorized {}: {}",
                            context.source_connection_id(),
                            err
                        );
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
        _sender: &dyn MessageSender<Self::Source>,
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
                warn!(
                    "Ignoring authorization complete message from connection {}: {}",
                    context.source_connection_id(),
                    err
                );
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
