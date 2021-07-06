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

use crate::network::dispatch::{
    ConnectionId, DispatchError, Handler, MessageContext, MessageSender,
};
use crate::protocol::authorization::{
    AuthComplete, AuthProtocolRequest, AuthProtocolResponse, AuthTrustRequest, AuthTrustResponse,
    AuthorizationError, PeerAuthorizationType,
};
use crate::protocol::network::NetworkMessage;
use crate::protocol::{PEER_AUTHORIZATION_PROTOCOL_MIN, PEER_AUTHORIZATION_PROTOCOL_VERSION};
use crate::protos::authorization;
use crate::protos::network;
use crate::protos::prelude::*;

use crate::network::auth::{
    state_machine::trust_v1::{
        TrustAuthorizationLocalAction, TrustAuthorizationRemoteAction,
        TrustAuthorizationRemoteState,
    },
    AuthorizationLocalAction, AuthorizationLocalState, AuthorizationManagerStateMachine,
    AuthorizationMessage, AuthorizationRemoteAction, AuthorizationRemoteState, Identity,
};

/// Handler for the Authorization Protocol Request Message Type
pub struct AuthProtocolRequestHandler {
    auth_manager: AuthorizationManagerStateMachine,
}

impl AuthProtocolRequestHandler {
    pub fn new(auth_manager: AuthorizationManagerStateMachine) -> Self {
        Self { auth_manager }
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
                    let response = AuthorizationMessage::AuthorizationError(
                        AuthorizationError::AuthorizationRejected(
                            "Unable to agree on protocol version".into(),
                        ),
                    );

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
                            AuthorizationRemoteAction::Unauthorizing,
                        )
                        .is_err()
                    {
                        warn!(
                            "Unable to update state to Unauthorizing for {}",
                            context.source_connection_id(),
                        )
                    };

                    return Ok(());
                };

                debug!(
                    "Sending agreed upon protocol version: {} and authorization types",
                    version
                );

                let response = AuthorizationMessage::AuthProtocolResponse(AuthProtocolResponse {
                    auth_protocol: version,
                    accepted_authorization_type: vec![PeerAuthorizationType::Trust],
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
    identity: String,
}

impl AuthProtocolResponseHandler {
    pub fn new(auth_manager: AuthorizationManagerStateMachine, identity: String) -> Self {
        Self {
            auth_manager,
            identity,
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
                if protocol_request
                    .accepted_authorization_type
                    .iter()
                    .any(|t| matches!(t, PeerAuthorizationType::Trust))
                {
                    let trust_request = AuthorizationMessage::AuthTrustRequest(AuthTrustRequest {
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

                    let msg_bytes = IntoBytes::<network::NetworkMessage>::into_bytes(
                        NetworkMessage::from(trust_request),
                    )?;

                    sender
                        .send(context.source_id().clone(), msg_bytes)
                        .map_err(|(recipient, payload)| {
                            DispatchError::NetworkSendError((recipient.into(), payload))
                        })?;
                }
            }
            Ok(next_state) => panic!("Should not have been able to transition to {}", next_state),
        }
        Ok(())
    }
}

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
