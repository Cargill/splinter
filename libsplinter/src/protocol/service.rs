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

//! Protocol structs for splinter service component messages
//!
//! These structs are used to operate on the messages that are transmitted between service
//! components and a splinter node.

use crate::protos::prelude::*;
use crate::protos::service;

/// A message envelope for messages either sent or received from service components.
pub struct ServiceMessage {
    /// The name of the circuit the message is meant for
    pub circuit: String,
    /// The unique ID of the service that is connecting to the circuit
    pub service_id: String,
    /// The message envelope contents
    pub payload: ServiceMessagePayload,
}

/// The payload of a service message.
pub enum ServiceMessagePayload {
    ConnectRequest(ServiceConnectRequest),
    ConnectResponse(ServiceConnectResponse),
    DisconnectRequest(ServiceDisconnectRequest),
    DisconnectResponse(ServiceDisconnectResponse),
    ServiceProcessorMessage(ServiceProcessorMessage),
}

/// This message is sent by a service processor component to connect to a splinter node.
pub struct ServiceConnectRequest {
    /// ID used to correlate the response with this request
    pub correlation_id: String,
}

/// This message is sent to a service processor component from a splinter node in response to its
/// connection request.
pub struct ServiceConnectResponse {
    /// ID used to correlate the response with this request
    pub correlation_id: String,
    /// The response status.
    pub status: ConnectResponseStatus,
}

pub enum ConnectResponseStatus {
    Ok,
    CircuitDoesNotExist(String),
    ServiceNotInCircuitRegistry(String),
    ServiceAlreadyRegistered(String),
    NotAnAllowedNode(String),
    InternalError(String),
    QueueFull,
}

/// This message is sent by a service processor component to disconnect from a splinter node.
pub struct ServiceDisconnectRequest {
    /// ID used to correlate the response with this request
    pub correlation_id: String,
}

/// This message is sent to a service processor component from a splinter node in response to its
/// disconnect request.
pub struct ServiceDisconnectResponse {
    /// ID used to correlate the response with this request
    pub correlation_id: String,
    /// The response status.
    pub status: DisconnectResponseStatus,
}

pub enum DisconnectResponseStatus {
    Ok,
    CircuitDoesNotExist(String),
    ServiceNotInCircuitRegistry(String),
    ServiceNotRegistered(String),
    QueueFull,
    InternalError(String),
}

/// Opaque messages that are sent to or received from a service processor.
pub struct ServiceProcessorMessage {
    /// The sending node
    pub sender: String,
    /// The target node
    pub recipient: String,
    /// The opaque payload.
    pub payload: Vec<u8>,
}

impl FromProto<service::SMConnectRequest> for ServiceConnectRequest {
    fn from_proto(mut req: service::SMConnectRequest) -> Result<Self, ProtoConversionError> {
        Ok(Self {
            correlation_id: req.take_correlation_id(),
        })
    }
}

impl FromNative<ServiceConnectRequest> for service::SMConnectRequest {
    fn from_native(req: ServiceConnectRequest) -> Result<Self, ProtoConversionError> {
        let mut proto_req = service::SMConnectRequest::new();
        proto_req.set_correlation_id(req.correlation_id);

        Ok(proto_req)
    }
}

impl FromProto<service::SMConnectResponse> for ServiceConnectResponse {
    fn from_proto(mut res: service::SMConnectResponse) -> Result<Self, ProtoConversionError> {
        use service::SMConnectResponse_Status::*;

        Ok(Self {
            correlation_id: res.take_correlation_id(),
            status: match res.get_status() {
                OK => ConnectResponseStatus::Ok,
                ERROR_CIRCUIT_DOES_NOT_EXIST => {
                    ConnectResponseStatus::CircuitDoesNotExist(res.take_error_message())
                }
                ERROR_SERVICE_NOT_IN_CIRCUIT_REGISTRY => {
                    ConnectResponseStatus::ServiceNotInCircuitRegistry(res.take_error_message())
                }
                ERROR_SERVICE_ALREADY_REGISTERED => {
                    ConnectResponseStatus::ServiceAlreadyRegistered(res.take_error_message())
                }
                ERROR_NOT_AN_ALLOWED_NODE => {
                    ConnectResponseStatus::NotAnAllowedNode(res.take_error_message())
                }
                ERROR_QUEUE_FULL => ConnectResponseStatus::QueueFull,
                ERROR_INTERNAL_ERROR => {
                    ConnectResponseStatus::InternalError(res.take_error_message())
                }
                UNSET_STATUS => {
                    return Err(ProtoConversionError::InvalidTypeError(
                        "no status was set".into(),
                    ))
                }
            },
        })
    }
}

impl FromNative<ServiceConnectResponse> for service::SMConnectResponse {
    fn from_native(res: ServiceConnectResponse) -> Result<Self, ProtoConversionError> {
        let mut proto_res = service::SMConnectResponse::new();
        proto_res.set_correlation_id(res.correlation_id);

        use service::SMConnectResponse_Status::*;
        match res.status {
            ConnectResponseStatus::Ok => proto_res.set_status(OK),
            ConnectResponseStatus::CircuitDoesNotExist(msg) => {
                proto_res.set_status(ERROR_CIRCUIT_DOES_NOT_EXIST);
                proto_res.set_error_message(msg);
            }
            ConnectResponseStatus::ServiceNotInCircuitRegistry(msg) => {
                proto_res.set_status(ERROR_SERVICE_NOT_IN_CIRCUIT_REGISTRY);
                proto_res.set_error_message(msg);
            }
            ConnectResponseStatus::ServiceAlreadyRegistered(msg) => {
                proto_res.set_status(ERROR_SERVICE_ALREADY_REGISTERED);
                proto_res.set_error_message(msg);
            }
            ConnectResponseStatus::NotAnAllowedNode(msg) => {
                proto_res.set_status(ERROR_NOT_AN_ALLOWED_NODE);
                proto_res.set_error_message(msg);
            }
            ConnectResponseStatus::InternalError(msg) => {
                proto_res.set_status(ERROR_INTERNAL_ERROR);
                proto_res.set_error_message(msg);
            }
            ConnectResponseStatus::QueueFull => proto_res.set_status(ERROR_QUEUE_FULL),
        }
        Ok(proto_res)
    }
}

impl FromProto<service::SMDisconnectRequest> for ServiceDisconnectRequest {
    fn from_proto(mut req: service::SMDisconnectRequest) -> Result<Self, ProtoConversionError> {
        Ok(Self {
            correlation_id: req.take_correlation_id(),
        })
    }
}

impl FromNative<ServiceDisconnectRequest> for service::SMDisconnectRequest {
    fn from_native(req: ServiceDisconnectRequest) -> Result<Self, ProtoConversionError> {
        let mut proto_req = service::SMDisconnectRequest::new();
        proto_req.set_correlation_id(req.correlation_id);

        Ok(proto_req)
    }
}

impl FromProto<service::SMDisconnectResponse> for ServiceDisconnectResponse {
    fn from_proto(mut res: service::SMDisconnectResponse) -> Result<Self, ProtoConversionError> {
        use service::SMDisconnectResponse_Status::*;

        Ok(Self {
            correlation_id: res.take_correlation_id(),
            status: match res.get_status() {
                OK => DisconnectResponseStatus::Ok,
                ERROR_CIRCUIT_DOES_NOT_EXIST => {
                    DisconnectResponseStatus::CircuitDoesNotExist(res.take_error_message())
                }
                ERROR_SERVICE_NOT_IN_CIRCUIT_REGISTRY => {
                    DisconnectResponseStatus::ServiceNotInCircuitRegistry(res.take_error_message())
                }
                ERROR_SERVICE_NOT_REGISTERED => {
                    DisconnectResponseStatus::ServiceNotRegistered(res.take_error_message())
                }
                ERROR_INTERNAL_ERROR => {
                    DisconnectResponseStatus::InternalError(res.take_error_message())
                }
                ERROR_QUEUE_FULL => DisconnectResponseStatus::QueueFull,
                UNSET_STATUS => {
                    return Err(ProtoConversionError::InvalidTypeError(
                        "no status was set".into(),
                    ))
                }
            },
        })
    }
}

impl FromNative<ServiceDisconnectResponse> for service::SMDisconnectResponse {
    fn from_native(res: ServiceDisconnectResponse) -> Result<Self, ProtoConversionError> {
        let mut proto_res = service::SMDisconnectResponse::new();
        proto_res.set_correlation_id(res.correlation_id);

        use service::SMDisconnectResponse_Status::*;
        match res.status {
            DisconnectResponseStatus::Ok => proto_res.set_status(OK),
            DisconnectResponseStatus::CircuitDoesNotExist(msg) => {
                proto_res.set_status(ERROR_CIRCUIT_DOES_NOT_EXIST);
                proto_res.set_error_message(msg);
            }
            DisconnectResponseStatus::ServiceNotInCircuitRegistry(msg) => {
                proto_res.set_status(ERROR_SERVICE_NOT_IN_CIRCUIT_REGISTRY);
                proto_res.set_error_message(msg);
            }
            DisconnectResponseStatus::ServiceNotRegistered(msg) => {
                proto_res.set_status(ERROR_SERVICE_NOT_REGISTERED);
                proto_res.set_error_message(msg);
            }
            DisconnectResponseStatus::InternalError(msg) => {
                proto_res.set_status(ERROR_INTERNAL_ERROR);
                proto_res.set_error_message(msg);
            }
            DisconnectResponseStatus::QueueFull => proto_res.set_status(ERROR_QUEUE_FULL),
        }
        Ok(proto_res)
    }
}

impl FromProto<service::ServiceProcessorMessage> for ServiceProcessorMessage {
    fn from_proto(mut msg: service::ServiceProcessorMessage) -> Result<Self, ProtoConversionError> {
        Ok(Self {
            sender: msg.take_sender(),
            recipient: msg.take_recipient(),
            payload: msg.take_payload(),
        })
    }
}

impl FromNative<ServiceProcessorMessage> for service::ServiceProcessorMessage {
    fn from_native(msg: ServiceProcessorMessage) -> Result<Self, ProtoConversionError> {
        let mut proto_msg = service::ServiceProcessorMessage::new();
        proto_msg.set_sender(msg.sender);
        proto_msg.set_recipient(msg.recipient);
        proto_msg.set_payload(msg.payload);

        Ok(proto_msg)
    }
}

impl FromProto<service::ServiceMessage> for ServiceMessage {
    fn from_proto(mut msg: service::ServiceMessage) -> Result<Self, ProtoConversionError> {
        let circuit = msg.take_circuit();
        let service_id = msg.take_service_id();

        use service::ServiceMessageType::*;
        let bytes = msg.get_payload();
        let payload = match msg.get_message_type() {
            SM_SERVICE_CONNECT_REQUEST => {
                ServiceMessagePayload::ConnectRequest(
                    FromBytes::<service::SMConnectRequest>::from_bytes(bytes)?,
                )
            }
            SM_SERVICE_CONNECT_RESPONSE => {
                ServiceMessagePayload::ConnectResponse(
                    FromBytes::<service::SMConnectResponse>::from_bytes(bytes)?,
                )
            }
            SM_SERVICE_DISCONNECT_REQUEST => {
                ServiceMessagePayload::DisconnectRequest(
                    FromBytes::<service::SMDisconnectRequest>::from_bytes(bytes)?,
                )
            }
            SM_SERVICE_DISCONNECT_RESPONSE => ServiceMessagePayload::DisconnectResponse(
                FromBytes::<service::SMDisconnectResponse>::from_bytes(bytes)?,
            ),
            SM_SERVICE_PROCESSOR_MESSAGE => {
                ServiceMessagePayload::ServiceProcessorMessage(FromBytes::<
                    service::ServiceProcessorMessage,
                >::from_bytes(bytes)?)
            }
            UNSET_SERVICE_MESSAGE_TYPE => {
                return Err(ProtoConversionError::InvalidTypeError(
                    "missing message type".into(),
                ));
            }
        };

        Ok(Self {
            circuit,
            service_id,
            payload,
        })
    }
}

impl FromNative<ServiceMessage> for service::ServiceMessage {
    fn from_native(msg: ServiceMessage) -> Result<Self, ProtoConversionError> {
        let mut proto_msg = service::ServiceMessage::new();
        proto_msg.set_circuit(msg.circuit);
        proto_msg.set_service_id(msg.service_id);

        use service::ServiceMessageType::*;
        match msg.payload {
            ServiceMessagePayload::ConnectRequest(req) => {
                proto_msg.set_message_type(SM_SERVICE_CONNECT_REQUEST);
                proto_msg.set_payload(IntoBytes::<service::SMConnectRequest>::into_bytes(req)?);
            }
            ServiceMessagePayload::ConnectResponse(req) => {
                proto_msg.set_message_type(SM_SERVICE_CONNECT_RESPONSE);
                proto_msg.set_payload(IntoBytes::<service::SMConnectResponse>::into_bytes(req)?);
            }
            ServiceMessagePayload::DisconnectRequest(req) => {
                proto_msg.set_message_type(SM_SERVICE_DISCONNECT_REQUEST);
                proto_msg.set_payload(IntoBytes::<service::SMDisconnectRequest>::into_bytes(req)?);
            }
            ServiceMessagePayload::DisconnectResponse(req) => {
                proto_msg.set_message_type(SM_SERVICE_DISCONNECT_RESPONSE);
                proto_msg.set_payload(IntoBytes::<service::SMDisconnectResponse>::into_bytes(req)?);
            }
            ServiceMessagePayload::ServiceProcessorMessage(msg) => {
                proto_msg.set_message_type(SM_SERVICE_PROCESSOR_MESSAGE);
                proto_msg.set_payload(IntoBytes::<service::ServiceProcessorMessage>::into_bytes(
                    msg,
                )?);
            }
        }

        Ok(proto_msg)
    }
}
