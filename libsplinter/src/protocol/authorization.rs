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

//! Protocol structs for splinter connection authorization.
//!
//! These structs are used to operate on the messages that are sent and received during the
//! authorization process for connections.

use crate::protos::authorization;
use crate::protos::prelude::*;

/// The authorization message envelope.
#[derive(Debug)]
pub enum AuthorizationMessage {
    ConnectRequest(ConnectRequest),
    ConnectResponse(ConnectResponse),
    Authorized(Authorized),
    AuthorizationError(AuthorizationError),

    TrustRequest(TrustRequest),
}

/// The possible types of authorization that may be computed during the handshake.
#[derive(Debug)]
pub enum AuthorizationType {
    Trust,
}

/// A connection request message.
///
/// This message provides information from the incoming connection.
#[derive(Debug)]
pub enum ConnectRequest {
    Bidirectional,
    Unidirectional,
}

/// A connection response message.
///
/// This message provides information for the incoming peer regarding the types of authorization
/// accepted.
#[derive(Debug)]
pub struct ConnectResponse {
    pub accepted_authorization_types: Vec<AuthorizationType>,
}

/// A trust request.
///
/// A trust request is sent in response to a Connect Message, if the node is using trust
/// authentication as its means of allowing a node to connect.
#[derive(Debug)]
pub struct TrustRequest {
    pub identity: String,
}

/// A successful authorization message.
///
/// This message is returned after either a TrustResponse has been returned by the remote
/// connection.
#[derive(Debug)]
pub struct Authorized;

/// A message indicating an error in authorization.
///
/// This includes failed authorizations, or invalid messages during the authorization handshake
/// conversation.
#[derive(Debug)]
pub enum AuthorizationError {
    AuthorizationRejected(String),
}

impl FromProto<authorization::ConnectRequest> for ConnectRequest {
    fn from_proto(source: authorization::ConnectRequest) -> Result<Self, ProtoConversionError> {
        use authorization::ConnectRequest_HandshakeMode::*;
        match source.handshake_mode {
            BIDIRECTIONAL => Ok(ConnectRequest::Bidirectional),
            UNIDIRECTIONAL => Ok(ConnectRequest::Unidirectional),
            UNSET_HANDSHAKE_MODE => Err(ProtoConversionError::InvalidTypeError(
                "No handshake mode was set".into(),
            )),
        }
    }
}

impl FromNative<ConnectRequest> for authorization::ConnectRequest {
    fn from_native(req: ConnectRequest) -> Result<Self, ProtoConversionError> {
        let mut proto_request = authorization::ConnectRequest::new();
        use authorization::ConnectRequest_HandshakeMode::*;
        proto_request.set_handshake_mode(match req {
            ConnectRequest::Bidirectional => BIDIRECTIONAL,
            ConnectRequest::Unidirectional => UNIDIRECTIONAL,
        });
        Ok(proto_request)
    }
}

impl FromProto<authorization::ConnectResponse> for ConnectResponse {
    fn from_proto(source: authorization::ConnectResponse) -> Result<Self, ProtoConversionError> {
        use authorization::ConnectResponse_AuthorizationType::*;
        Ok(Self {
            accepted_authorization_types: source
                .get_accepted_authorization_types()
                .iter()
                .map(|t| match t {
                    TRUST => Ok(AuthorizationType::Trust),
                    UNSET_AUTHORIZATION_TYPE => Err(ProtoConversionError::InvalidTypeError(
                        "no authorization type was set".into(),
                    )),
                })
                .collect::<Result<Vec<AuthorizationType>, ProtoConversionError>>()?,
        })
    }
}

impl FromNative<ConnectResponse> for authorization::ConnectResponse {
    fn from_native(source: ConnectResponse) -> Result<Self, ProtoConversionError> {
        let mut response = authorization::ConnectResponse::new();

        response.set_accepted_authorization_types(
            source
                .accepted_authorization_types
                .into_iter()
                .map(|auth_type| match auth_type {
                    AuthorizationType::Trust => {
                        authorization::ConnectResponse_AuthorizationType::TRUST
                    }
                })
                .collect(),
        );

        Ok(response)
    }
}

impl FromProto<authorization::TrustRequest> for TrustRequest {
    fn from_proto(mut source: authorization::TrustRequest) -> Result<Self, ProtoConversionError> {
        Ok(Self {
            identity: source.take_identity(),
        })
    }
}

impl FromNative<TrustRequest> for authorization::TrustRequest {
    fn from_native(source: TrustRequest) -> Result<Self, ProtoConversionError> {
        let mut request = authorization::TrustRequest::new();
        request.set_identity(source.identity);

        Ok(request)
    }
}

impl FromProto<authorization::AuthorizedMessage> for Authorized {
    fn from_proto(_: authorization::AuthorizedMessage) -> Result<Self, ProtoConversionError> {
        Ok(Authorized)
    }
}

impl FromNative<Authorized> for authorization::AuthorizedMessage {
    fn from_native(_: Authorized) -> Result<Self, ProtoConversionError> {
        Ok(authorization::AuthorizedMessage::new())
    }
}

impl FromProto<authorization::AuthorizationError> for AuthorizationError {
    fn from_proto(
        mut source: authorization::AuthorizationError,
    ) -> Result<Self, ProtoConversionError> {
        use authorization::AuthorizationError_AuthorizationErrorType::*;
        match source.error_type {
            AUTHORIZATION_REJECTED => Ok(AuthorizationError::AuthorizationRejected(
                source.take_error_message(),
            )),
            UNSET_AUTHORIZATION_ERROR_TYPE => Err(ProtoConversionError::InvalidTypeError(
                "No error type set".into(),
            )),
        }
    }
}

impl FromNative<AuthorizationError> for authorization::AuthorizationError {
    fn from_native(source: AuthorizationError) -> Result<Self, ProtoConversionError> {
        use authorization::AuthorizationError_AuthorizationErrorType::*;
        let mut error = authorization::AuthorizationError::new();
        match source {
            AuthorizationError::AuthorizationRejected(message) => {
                error.set_error_type(AUTHORIZATION_REJECTED);
                error.set_error_message(message);
            }
        }
        Ok(error)
    }
}

impl FromProto<authorization::AuthorizationMessage> for AuthorizationMessage {
    fn from_proto(
        source: authorization::AuthorizationMessage,
    ) -> Result<Self, ProtoConversionError> {
        use authorization::AuthorizationMessageType::*;
        match source.message_type {
            CONNECT_REQUEST => Ok(AuthorizationMessage::ConnectRequest(FromBytes::<
                authorization::ConnectRequest,
            >::from_bytes(
                source.get_payload(),
            )?)),
            CONNECT_RESPONSE => Ok(AuthorizationMessage::ConnectResponse(FromBytes::<
                authorization::ConnectResponse,
            >::from_bytes(
                source.get_payload(),
            )?)),
            AUTHORIZE => Ok(AuthorizationMessage::Authorized(FromBytes::<
                authorization::AuthorizedMessage,
            >::from_bytes(
                source.get_payload()
            )?)),
            AUTHORIZATION_ERROR => Ok(AuthorizationMessage::AuthorizationError(FromBytes::<
                authorization::AuthorizationError,
            >::from_bytes(
                source.get_payload(),
            )?)),
            TRUST_REQUEST => Ok(AuthorizationMessage::TrustRequest(FromBytes::<
                authorization::TrustRequest,
            >::from_bytes(
                source.get_payload()
            )?)),
            UNSET_AUTHORIZATION_MESSAGE_TYPE => Err(ProtoConversionError::InvalidTypeError(
                "no message type was set".into(),
            )),
        }
    }
}

impl FromNative<AuthorizationMessage> for authorization::AuthorizationMessage {
    fn from_native(source: AuthorizationMessage) -> Result<Self, ProtoConversionError> {
        use authorization::AuthorizationMessageType::*;

        let mut message = authorization::AuthorizationMessage::new();
        match source {
            AuthorizationMessage::ConnectRequest(payload) => {
                message.set_message_type(CONNECT_REQUEST);
                message.set_payload(IntoBytes::<authorization::ConnectRequest>::into_bytes(
                    payload,
                )?);
            }
            AuthorizationMessage::ConnectResponse(payload) => {
                message.set_message_type(CONNECT_RESPONSE);
                message.set_payload(IntoBytes::<authorization::ConnectResponse>::into_bytes(
                    payload,
                )?);
            }
            AuthorizationMessage::Authorized(payload) => {
                message.set_message_type(AUTHORIZE);
                message.set_payload(IntoBytes::<authorization::AuthorizedMessage>::into_bytes(
                    payload,
                )?);
            }

            AuthorizationMessage::AuthorizationError(payload) => {
                message.set_message_type(AUTHORIZATION_ERROR);
                message.set_payload(IntoBytes::<authorization::AuthorizationError>::into_bytes(
                    payload,
                )?);
            }
            AuthorizationMessage::TrustRequest(payload) => {
                message.set_message_type(TRUST_REQUEST);
                message.set_payload(IntoBytes::<authorization::TrustRequest>::into_bytes(
                    payload,
                )?);
            }
        }
        Ok(message)
    }
}
