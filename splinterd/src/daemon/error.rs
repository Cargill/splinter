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

use std::error::Error;
use std::fmt;

use splinter::error::InternalError;
use splinter::transport::{AcceptError, ConnectError, ListenError};
use splinter_rest_api_common::error::RestApiServerError;

use crate::error::UserError;

#[derive(Debug)]
pub enum CreateError {
    MissingRequiredField(String),
    InvalidArgument(String),
}

impl Error for CreateError {}

impl fmt::Display for CreateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CreateError::MissingRequiredField(msg) => write!(f, "missing required field: {}", msg),
            CreateError::InvalidArgument(msg) => f.write_str(msg),
        }
    }
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
pub enum StartError {
    TransportError(String),
    NetworkError(String),
    StorageError(String),
    ProtocolError(String),
    RestApiError(String),
    AdminServiceError(String),
    #[cfg(feature = "health-service")]
    HealthServiceError(String),
    OrchestratorError(String),
    UserError(String),
    InternalError(String),
}

impl Error for StartError {}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StartError::TransportError(msg) => write!(f, "transport returned an error: {}", msg),
            StartError::NetworkError(msg) => write!(f, "network returned an error: {}", msg),
            StartError::StorageError(msg) => write!(f, "unable to set up storage: {}", msg),
            StartError::ProtocolError(msg) => write!(f, "unable to parse protocol: {}", msg),
            StartError::RestApiError(msg) => write!(f, "REST API encountered an error: {}", msg),
            StartError::AdminServiceError(msg) => {
                write!(f, "the admin service encountered an error: {}", msg)
            }
            #[cfg(feature = "health-service")]
            StartError::HealthServiceError(msg) => {
                write!(f, "the health service encountered an error: {}", msg)
            }
            StartError::OrchestratorError(msg) => {
                write!(f, "the orchestrator encountered an error: {}", msg)
            }
            StartError::UserError(msg) => {
                write!(f, "there has been a user error: {}", msg)
            }
            StartError::InternalError(err) => f.write_str(err),
        }
    }
}

impl From<UserError> for StartError {
    fn from(user_error: UserError) -> Self {
        StartError::UserError(user_error.to_string())
    }
}

impl From<RestApiServerError> for StartError {
    fn from(rest_api_error: RestApiServerError) -> Self {
        StartError::RestApiError(rest_api_error.to_string())
    }
}

impl From<ListenError> for StartError {
    fn from(listen_error: ListenError) -> Self {
        StartError::TransportError(format!("Listen Error: {:?}", listen_error))
    }
}

impl From<AcceptError> for StartError {
    fn from(accept_error: AcceptError) -> Self {
        StartError::TransportError(format!("Accept Error: {:?}", accept_error))
    }
}

impl From<ConnectError> for StartError {
    fn from(connect_error: ConnectError) -> Self {
        StartError::TransportError(format!("Connect Error: {:?}", connect_error))
    }
}

impl From<protobuf::ProtobufError> for StartError {
    fn from(err: protobuf::ProtobufError) -> Self {
        StartError::ProtocolError(err.to_string())
    }
}

impl From<InternalError> for StartError {
    fn from(err: InternalError) -> Self {
        StartError::InternalError(err.to_string())
    }
}
