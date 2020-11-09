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

use std::error::Error;
use std::fmt;

use crate::admin::store::error::AdminServiceStoreError;
use crate::consensus::error::ProposalManagerError;
use crate::orchestrator::InitializeServiceError;
use crate::service::error::{ServiceError, ServiceSendError};

use protobuf::error;

#[derive(Debug)]
pub enum AdminServiceError {
    ServiceError(ServiceError),

    GeneralError {
        context: String,
        source: Option<Box<dyn Error + Send>>,
    },
}

impl AdminServiceError {
    pub fn general_error(context: &str) -> Self {
        AdminServiceError::GeneralError {
            context: context.into(),
            source: None,
        }
    }

    pub fn general_error_with_source(context: &str, err: Box<dyn Error + Send>) -> Self {
        AdminServiceError::GeneralError {
            context: context.into(),
            source: Some(err),
        }
    }
}

impl Error for AdminServiceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AdminServiceError::ServiceError(err) => Some(err),
            AdminServiceError::GeneralError { source, .. } => {
                if let Some(ref err) = source {
                    Some(&**err)
                } else {
                    None
                }
            }
        }
    }
}

impl fmt::Display for AdminServiceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AdminServiceError::ServiceError(err) => f.write_str(&err.to_string()),
            AdminServiceError::GeneralError { context, source } => {
                if let Some(ref err) = source {
                    write!(f, "{}: {}", context, err)
                } else {
                    f.write_str(&context)
                }
            }
        }
    }
}

impl From<ServiceError> for AdminServiceError {
    fn from(err: ServiceError) -> Self {
        AdminServiceError::ServiceError(err)
    }
}

#[derive(Debug)]
pub enum AdminSubscriberError {
    UnableToHandleEvent(String),
    Unsubscribe,
}

impl Error for AdminSubscriberError {}

impl fmt::Display for AdminSubscriberError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AdminSubscriberError::UnableToHandleEvent(msg) => {
                write!(f, "Unable to handle event: {}", msg)
            }
            AdminSubscriberError::Unsubscribe => f.write_str("Unsubscribe"),
        }
    }
}

#[derive(Debug)]
pub struct AdminKeyVerifierError {
    context: String,
    source: Option<Box<dyn Error + Send>>,
}

impl AdminKeyVerifierError {
    pub fn new(context: &str) -> Self {
        Self {
            context: context.into(),
            source: None,
        }
    }

    pub fn new_with_source(context: &str, err: Box<dyn Error + Send>) -> Self {
        Self {
            context: context.into(),
            source: Some(err),
        }
    }
}

impl Error for AdminKeyVerifierError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if let Some(ref err) = self.source {
            Some(&**err)
        } else {
            None
        }
    }
}

impl fmt::Display for AdminKeyVerifierError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref err) = self.source {
            write!(f, "{}: {}", self.context, err)
        } else {
            f.write_str(&self.context)
        }
    }
}

impl From<ServiceError> for ProposalManagerError {
    fn from(err: ServiceError) -> Self {
        ProposalManagerError::Internal(Box::new(err))
    }
}

#[derive(Debug)]
pub enum AdminSharedError {
    SplinterStateError(String),
    HashError(Sha256Error),
    InvalidMessageFormat(MarshallingError),
    NoPendingChanges,
    ServiceInitializationFailed {
        context: String,
        source: Option<InitializeServiceError>,
    },
    ServiceSendError(ServiceSendError),
    UnknownAction(String),
    ValidationFailed(String),

    /// An error occurred while trying to add an admin service event subscriber to the service.
    UnableToAddSubscriber(String),

    // An error occured while trying to negotiated protocol versions
    ServiceProtocolError(String),
}

impl Error for AdminSharedError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AdminSharedError::SplinterStateError(_) => None,
            AdminSharedError::HashError(err) => Some(err),
            AdminSharedError::InvalidMessageFormat(err) => Some(err),
            AdminSharedError::NoPendingChanges => None,
            AdminSharedError::ServiceInitializationFailed { source, .. } => {
                if let Some(ref err) = source {
                    Some(err)
                } else {
                    None
                }
            }
            AdminSharedError::ServiceSendError(err) => Some(err),
            AdminSharedError::UnknownAction(_) => None,
            AdminSharedError::ValidationFailed(_) => None,
            AdminSharedError::UnableToAddSubscriber(_) => None,
            AdminSharedError::ServiceProtocolError(_) => None,
        }
    }
}

impl fmt::Display for AdminSharedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AdminSharedError::SplinterStateError(msg) => write!(f, "error using store {}", msg),
            AdminSharedError::HashError(err) => write!(f, "received error while hashing: {}", err),
            AdminSharedError::InvalidMessageFormat(err) => {
                write!(f, "invalid message format: {}", err)
            }
            AdminSharedError::NoPendingChanges => {
                write!(f, "tried to commit without pending changes")
            }
            AdminSharedError::ServiceInitializationFailed { context, source } => {
                if let Some(ref err) = source {
                    write!(f, "{}: {}", context, err)
                } else {
                    f.write_str(&context)
                }
            }
            AdminSharedError::ServiceSendError(err) => {
                write!(f, "failed to send service message: {}", err)
            }
            AdminSharedError::UnknownAction(msg) => {
                write!(f, "received message with unknown action: {}", msg)
            }
            AdminSharedError::ValidationFailed(msg) => write!(f, "validation failed: {}", msg),
            AdminSharedError::UnableToAddSubscriber(msg) => {
                write!(f, "unable to add admin service event subscriber: {}", msg)
            }
            AdminSharedError::ServiceProtocolError(msg) => write!(
                f,
                "error occured while trying to agree on protocol: {}",
                msg
            ),
        }
    }
}

impl From<ServiceSendError> for AdminSharedError {
    fn from(err: ServiceSendError) -> Self {
        AdminSharedError::ServiceSendError(err)
    }
}

impl From<MarshallingError> for AdminSharedError {
    fn from(err: MarshallingError) -> Self {
        AdminSharedError::InvalidMessageFormat(err)
    }
}

impl From<AdminServiceStoreError> for AdminSharedError {
    fn from(err: AdminServiceStoreError) -> Self {
        AdminSharedError::SplinterStateError(err.to_string())
    }
}

impl From<AdminKeyVerifierError> for AdminSharedError {
    fn from(err: AdminKeyVerifierError) -> Self {
        AdminSharedError::ValidationFailed(format!("unable to verify key permissions: {}", err))
    }
}

#[derive(Debug)]
pub struct AdminConsensusManagerError(pub Box<dyn Error + Send>);

impl Error for AdminConsensusManagerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&*self.0)
    }
}

impl std::fmt::Display for AdminConsensusManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "admin consensus manager failed: {}", self.0)
    }
}

#[derive(Debug)]
pub enum AdminError {
    ConsensusFailed(AdminConsensusManagerError),
    MessageTypeUnset,
}

impl Error for AdminError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AdminError::ConsensusFailed(err) => Some(err),
            AdminError::MessageTypeUnset => None,
        }
    }
}

impl std::fmt::Display for AdminError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            AdminError::ConsensusFailed(err) => write!(f, "admin consensus failed: {}", err),
            AdminError::MessageTypeUnset => write!(f, "received message with unset type"),
        }
    }
}

impl From<AdminConsensusManagerError> for AdminError {
    fn from(err: AdminConsensusManagerError) -> Self {
        AdminError::ConsensusFailed(err)
    }
}

#[derive(Debug)]
pub struct Sha256Error(pub Box<dyn Error + Send>);

impl Error for Sha256Error {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&*self.0)
    }
}

impl std::fmt::Display for Sha256Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "unable to get sha256 hash: {}", self.0)
    }
}

impl From<Sha256Error> for AdminSharedError {
    fn from(err: Sha256Error) -> Self {
        AdminSharedError::HashError(err)
    }
}

#[derive(Debug)]
pub enum MarshallingError {
    UnsetField(String),
    ProtobufError(error::ProtobufError),
}

impl std::error::Error for MarshallingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MarshallingError::UnsetField(_) => None,
            MarshallingError::ProtobufError(err) => Some(err),
        }
    }
}

impl std::fmt::Display for MarshallingError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            MarshallingError::UnsetField(_) => write!(f, "Invalid enumerated type"),
            MarshallingError::ProtobufError(err) => write!(f, "Protobuf Error: {}", err),
        }
    }
}

impl From<error::ProtobufError> for MarshallingError {
    fn from(err: error::ProtobufError) -> Self {
        MarshallingError::ProtobufError(err)
    }
}
