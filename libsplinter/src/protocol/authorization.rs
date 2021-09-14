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

    // v1 messages
    AuthComplete(AuthComplete),
    AuthProtocolRequest(AuthProtocolRequest),
    AuthProtocolResponse(AuthProtocolResponse),

    AuthTrustRequest(AuthTrustRequest),
    AuthTrustResponse(AuthTrustResponse),

    AuthChallengeNonceRequest(AuthChallengeNonceRequest),
    AuthChallengeNonceResponse(AuthChallengeNonceResponse),
    AuthChallengeSubmitRequest(AuthChallengeSubmitRequest),
    AuthChallengeSubmitResponse(AuthChallengeSubmitResponse),
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

// ------------- v1 messages ----------

/// A successful authorization message.
///
/// This message is returned after Trust or Challenge authorization is completed
#[derive(Debug)]
pub struct AuthComplete;

/// A protocol request message.
///
/// This message provides supported protocol versions and requests that an agreed upon version is
/// returned.
#[derive(Debug)]
pub struct AuthProtocolRequest {
    pub auth_protocol_min: u32,
    pub auth_protocol_max: u32,
}

#[derive(Debug, Clone)]
pub enum PeerAuthorizationType {
    Trust,
    Challenge,
}

/// A protocol response message.
///
/// This message returns the agreed upon authorization protocol and a list of supported peer
/// authorization types.
#[derive(Debug)]
pub struct AuthProtocolResponse {
    pub auth_protocol: u32,
    pub accepted_authorization_type: Vec<PeerAuthorizationType>,
}

/// A trust request.
///
/// A trust request is sent to a node, if the other node accepts trust authorization a trust
/// response will be returned.
#[derive(Debug)]
pub struct AuthTrustRequest {
    pub identity: String,
}

/// A successful trust authorization.
///
/// This message is returned if trust a request is accepted
#[derive(Debug)]
pub struct AuthTrustResponse;

/// A challenge nounce request
///
/// This request is for a nonce that will be used to create the signature used in the
/// AuthChallengeSubmitRequest message
#[derive(Debug)]
pub struct AuthChallengeNonceRequest;

/// A challenge nounce response
///
/// This response contains nonce that must be used to create the signature used in the
/// AuthChallengeSubmitRequest message
#[derive(Debug)]
pub struct AuthChallengeNonceResponse {
    pub nonce: Vec<u8>,
}

#[derive(Debug)]
pub struct SubmitRequest {
    pub public_key: Vec<u8>,
    pub signature: Vec<u8>,
}

/// A challenge submit request
///
/// This request contains the signature created from the nonce in the AuthChallengeNonceResponse
/// and the public key for the signature.
#[derive(Debug)]
pub struct AuthChallengeSubmitRequest {
    pub submit_requests: Vec<SubmitRequest>,
}

/// A successful challenge authorization.
///
/// This message is returned if challenge submit request is accepted
#[derive(Debug)]
pub struct AuthChallengeSubmitResponse {
    pub public_key: Vec<u8>,
}

impl FromProto<authorization::AuthComplete> for AuthComplete {
    fn from_proto(_: authorization::AuthComplete) -> Result<Self, ProtoConversionError> {
        Ok(AuthComplete)
    }
}

impl FromNative<AuthComplete> for authorization::AuthComplete {
    fn from_native(_: AuthComplete) -> Result<Self, ProtoConversionError> {
        Ok(authorization::AuthComplete::new())
    }
}

impl FromProto<authorization::AuthProtocolRequest> for AuthProtocolRequest {
    fn from_proto(
        source: authorization::AuthProtocolRequest,
    ) -> Result<Self, ProtoConversionError> {
        Ok(AuthProtocolRequest {
            auth_protocol_min: source.get_auth_protocol_min(),
            auth_protocol_max: source.get_auth_protocol_max(),
        })
    }
}

impl FromNative<AuthProtocolRequest> for authorization::AuthProtocolRequest {
    fn from_native(req: AuthProtocolRequest) -> Result<Self, ProtoConversionError> {
        let mut proto_request = authorization::AuthProtocolRequest::new();
        proto_request.set_auth_protocol_min(req.auth_protocol_min);
        proto_request.set_auth_protocol_max(req.auth_protocol_max);
        Ok(proto_request)
    }
}

impl FromProto<authorization::AuthProtocolResponse> for AuthProtocolResponse {
    fn from_proto(
        source: authorization::AuthProtocolResponse,
    ) -> Result<Self, ProtoConversionError> {
        use authorization::AuthProtocolResponse_PeerAuthorizationType::*;
        Ok(AuthProtocolResponse {
            auth_protocol: source.get_auth_protocol(),
            accepted_authorization_type: source
                .get_accepted_authorization_type()
                .iter()
                .map(|auth_type| match auth_type {
                    UNSET_AUTHORIZATION_TYPE => Err(ProtoConversionError::InvalidTypeError(
                        "No handshake mode was set".into(),
                    )),
                    TRUST => Ok(PeerAuthorizationType::Trust),
                    CHALLENGE => Ok(PeerAuthorizationType::Challenge),
                })
                .collect::<Result<Vec<_>, ProtoConversionError>>()?,
        })
    }
}

impl FromNative<AuthProtocolResponse> for authorization::AuthProtocolResponse {
    fn from_native(req: AuthProtocolResponse) -> Result<Self, ProtoConversionError> {
        use authorization::AuthProtocolResponse_PeerAuthorizationType::*;

        let mut proto_request = authorization::AuthProtocolResponse::new();
        proto_request.set_auth_protocol(req.auth_protocol);
        proto_request.set_accepted_authorization_type(
            req.accepted_authorization_type
                .iter()
                .map(|auth_type| match auth_type {
                    PeerAuthorizationType::Trust => TRUST,
                    PeerAuthorizationType::Challenge => CHALLENGE,
                })
                .collect(),
        );
        Ok(proto_request)
    }
}

impl FromProto<authorization::AuthTrustRequest> for AuthTrustRequest {
    fn from_proto(
        mut source: authorization::AuthTrustRequest,
    ) -> Result<Self, ProtoConversionError> {
        Ok(AuthTrustRequest {
            identity: source.take_identity(),
        })
    }
}

impl FromNative<AuthTrustRequest> for authorization::AuthTrustRequest {
    fn from_native(req: AuthTrustRequest) -> Result<Self, ProtoConversionError> {
        let mut proto_request = authorization::AuthTrustRequest::new();
        proto_request.set_identity(req.identity);
        Ok(proto_request)
    }
}

impl FromProto<authorization::AuthTrustResponse> for AuthTrustResponse {
    fn from_proto(_: authorization::AuthTrustResponse) -> Result<Self, ProtoConversionError> {
        Ok(AuthTrustResponse)
    }
}

impl FromNative<AuthTrustResponse> for authorization::AuthTrustResponse {
    fn from_native(_: AuthTrustResponse) -> Result<Self, ProtoConversionError> {
        Ok(authorization::AuthTrustResponse::new())
    }
}

impl FromProto<authorization::AuthChallengeNonceRequest> for AuthChallengeNonceRequest {
    fn from_proto(
        _: authorization::AuthChallengeNonceRequest,
    ) -> Result<Self, ProtoConversionError> {
        Ok(AuthChallengeNonceRequest)
    }
}

impl FromNative<AuthChallengeNonceRequest> for authorization::AuthChallengeNonceRequest {
    fn from_native(_: AuthChallengeNonceRequest) -> Result<Self, ProtoConversionError> {
        Ok(authorization::AuthChallengeNonceRequest::new())
    }
}

impl FromProto<authorization::AuthChallengeNonceResponse> for AuthChallengeNonceResponse {
    fn from_proto(
        mut source: authorization::AuthChallengeNonceResponse,
    ) -> Result<Self, ProtoConversionError> {
        Ok(AuthChallengeNonceResponse {
            nonce: source.take_nonce(),
        })
    }
}

impl FromNative<AuthChallengeNonceResponse> for authorization::AuthChallengeNonceResponse {
    fn from_native(req: AuthChallengeNonceResponse) -> Result<Self, ProtoConversionError> {
        let mut proto_request = authorization::AuthChallengeNonceResponse::new();
        proto_request.set_nonce(req.nonce);
        Ok(proto_request)
    }
}

impl FromProto<authorization::AuthChallengeSubmitRequest> for AuthChallengeSubmitRequest {
    fn from_proto(
        mut source: authorization::AuthChallengeSubmitRequest,
    ) -> Result<Self, ProtoConversionError> {
        Ok(AuthChallengeSubmitRequest {
            submit_requests: source
                .take_submit_requests()
                .into_iter()
                .map(|mut submit_request| SubmitRequest {
                    public_key: submit_request.take_public_key(),
                    signature: submit_request.take_signature(),
                })
                .collect(),
        })
    }
}

impl FromNative<AuthChallengeSubmitRequest> for authorization::AuthChallengeSubmitRequest {
    fn from_native(req: AuthChallengeSubmitRequest) -> Result<Self, ProtoConversionError> {
        let mut proto_request = authorization::AuthChallengeSubmitRequest::new();
        let submit_requests = req
            .submit_requests
            .iter()
            .map(|submit_request| {
                let mut proto_submit_request = authorization::SubmitRequest::new();
                proto_submit_request.set_public_key(submit_request.public_key.to_vec());
                proto_submit_request.set_signature(submit_request.signature.to_vec());
                proto_submit_request
            })
            .collect();

        proto_request.set_submit_requests(submit_requests);
        Ok(proto_request)
    }
}

impl FromNative<AuthChallengeSubmitResponse> for authorization::AuthChallengeSubmitResponse {
    fn from_native(response: AuthChallengeSubmitResponse) -> Result<Self, ProtoConversionError> {
        let mut proto_response = authorization::AuthChallengeSubmitResponse::new();
        proto_response.set_public_key(response.public_key);
        Ok(proto_response)
    }
}

impl FromProto<authorization::AuthChallengeSubmitResponse> for AuthChallengeSubmitResponse {
    fn from_proto(
        mut source: authorization::AuthChallengeSubmitResponse,
    ) -> Result<Self, ProtoConversionError> {
        Ok(AuthChallengeSubmitResponse {
            public_key: source.take_public_key(),
        })
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
            AUTH_COMPLETE => Ok(AuthorizationMessage::AuthComplete(FromBytes::<
                authorization::AuthComplete,
            >::from_bytes(
                source.get_payload()
            )?)),
            AUTH_PROTOCOL_REQUEST => Ok(AuthorizationMessage::AuthProtocolRequest(FromBytes::<
                authorization::AuthProtocolRequest,
            >::from_bytes(
                source.get_payload(),
            )?)),
            AUTH_PROTOCOL_RESPONSE => Ok(AuthorizationMessage::AuthProtocolResponse(FromBytes::<
                authorization::AuthProtocolResponse,
            >::from_bytes(
                source.get_payload(),
            )?)),
            AUTH_TRUST_REQUEST => Ok(AuthorizationMessage::AuthTrustRequest(FromBytes::<
                authorization::AuthTrustRequest,
            >::from_bytes(
                source.get_payload(),
            )?)),
            AUTH_TRUST_RESPONSE => Ok(AuthorizationMessage::AuthTrustResponse(FromBytes::<
                authorization::AuthTrustResponse,
            >::from_bytes(
                source.get_payload(),
            )?)),
            AUTH_CHALLENGE_NONCE_REQUEST => Ok(AuthorizationMessage::AuthChallengeNonceRequest(
                FromBytes::<authorization::AuthChallengeNonceRequest>::from_bytes(
                    source.get_payload(),
                )?,
            )),
            AUTH_CHALLENGE_NONCE_RESPONSE => Ok(AuthorizationMessage::AuthChallengeNonceResponse(
                FromBytes::<authorization::AuthChallengeNonceResponse>::from_bytes(
                    source.get_payload(),
                )?,
            )),
            AUTH_CHALLENGE_SUBMIT_REQUEST => Ok(AuthorizationMessage::AuthChallengeSubmitRequest(
                FromBytes::<authorization::AuthChallengeSubmitRequest>::from_bytes(
                    source.get_payload(),
                )?,
            )),
            AUTH_CHALLENGE_SUBMIT_RESPONSE => {
                Ok(AuthorizationMessage::AuthChallengeSubmitResponse(
                    FromBytes::<authorization::AuthChallengeSubmitResponse>::from_bytes(
                        source.get_payload(),
                    )?,
                ))
            }
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
            AuthorizationMessage::AuthComplete(payload) => {
                message.set_message_type(AUTH_COMPLETE);
                message.set_payload(IntoBytes::<authorization::AuthComplete>::into_bytes(
                    payload,
                )?);
            }
            AuthorizationMessage::AuthProtocolRequest(payload) => {
                message.set_message_type(AUTH_PROTOCOL_REQUEST);
                message.set_payload(IntoBytes::<authorization::AuthProtocolRequest>::into_bytes(
                    payload,
                )?);
            }
            AuthorizationMessage::AuthProtocolResponse(payload) => {
                message.set_message_type(AUTH_PROTOCOL_RESPONSE);
                message.set_payload(
                    IntoBytes::<authorization::AuthProtocolResponse>::into_bytes(payload)?,
                );
            }
            AuthorizationMessage::AuthTrustRequest(payload) => {
                message.set_message_type(AUTH_TRUST_REQUEST);
                message.set_payload(IntoBytes::<authorization::AuthTrustRequest>::into_bytes(
                    payload,
                )?);
            }
            AuthorizationMessage::AuthTrustResponse(payload) => {
                message.set_message_type(AUTH_TRUST_RESPONSE);
                message.set_payload(IntoBytes::<authorization::AuthTrustResponse>::into_bytes(
                    payload,
                )?);
            }
            AuthorizationMessage::AuthChallengeNonceRequest(payload) => {
                message.set_message_type(AUTH_CHALLENGE_NONCE_REQUEST);
                message.set_payload(
                    IntoBytes::<authorization::AuthChallengeNonceRequest>::into_bytes(payload)?,
                );
            }
            AuthorizationMessage::AuthChallengeNonceResponse(payload) => {
                message.set_message_type(AUTH_CHALLENGE_NONCE_RESPONSE);
                message.set_payload(
                    IntoBytes::<authorization::AuthChallengeNonceResponse>::into_bytes(payload)?,
                );
            }
            AuthorizationMessage::AuthChallengeSubmitRequest(payload) => {
                message.set_message_type(AUTH_CHALLENGE_SUBMIT_REQUEST);
                message.set_payload(
                    IntoBytes::<authorization::AuthChallengeSubmitRequest>::into_bytes(payload)?,
                );
            }
            AuthorizationMessage::AuthChallengeSubmitResponse(payload) => {
                message.set_message_type(AUTH_CHALLENGE_SUBMIT_RESPONSE);
                message.set_payload(
                    IntoBytes::<authorization::AuthChallengeSubmitResponse>::into_bytes(payload)?,
                );
            }
        }
        Ok(message)
    }
}
