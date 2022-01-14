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

#[cfg(feature = "diesel")]
use crate::error::ConstraintViolationType;
use crate::error::{
    ConstraintViolationError, InternalError, InvalidStateError, ResourceTemporarilyUnavailableError,
};

/// Represents errors that occur with node registry operations
#[derive(Debug)]
pub enum RegistryError {
    /// Represents errors internal to the function.
    InternalError(InternalError),
    /// Represents constraint violations on the database's definition
    ConstraintViolationError(ConstraintViolationError),
    /// Represents when the underlying resource is unavailable
    ResourceTemporarilyUnavailableError(ResourceTemporarilyUnavailableError),
    /// Represents when an operation cannot be completed because the state of the underlying
    /// struct is inconsistent.
    InvalidStateError(InvalidStateError),
}

impl Error for RegistryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RegistryError::InternalError(err) => Some(err),
            RegistryError::ConstraintViolationError(err) => Some(err),
            RegistryError::ResourceTemporarilyUnavailableError(err) => Some(err),
            RegistryError::InvalidStateError(err) => Some(err),
        }
    }
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RegistryError::InternalError(err) => write!(f, "{}", err),
            RegistryError::ConstraintViolationError(err) => write!(f, "{}", err),
            RegistryError::ResourceTemporarilyUnavailableError(err) => {
                write!(f, "{}", err)
            }
            RegistryError::InvalidStateError(err) => write!(f, "{}", err),
        }
    }
}

#[cfg(feature = "diesel")]
impl From<diesel::r2d2::PoolError> for RegistryError {
    fn from(err: diesel::r2d2::PoolError) -> Self {
        RegistryError::ResourceTemporarilyUnavailableError(
            ResourceTemporarilyUnavailableError::from_source(Box::new(err)),
        )
    }
}

impl From<InternalError> for RegistryError {
    fn from(err: InternalError) -> Self {
        Self::InternalError(err)
    }
}

/// Represents the reason that a node was found to be invalid
#[derive(Debug)]
pub enum InvalidNodeError {
    /// One of the node's endpoints is already in use by another node
    DuplicateEndpoint(String),
    /// The node's identity is already in use by another node
    DuplicateIdentity(String),
    /// One of the node's endpoints is an empty string
    EmptyEndpoint,
    /// The node's identity is an empty string
    EmptyIdentity,
    /// The node's display name is an empty string
    EmptyDisplayName,
    /// One of the node's keys is an empty string
    EmptyKey,
    /// The node's identity is invalid (identity, message)
    InvalidIdentity(String, String),
    /// The node's list of endpoints is empty
    MissingEndpoints,
    /// The node's list of keys is empty
    MissingKeys,
}

impl Error for InvalidNodeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            InvalidNodeError::DuplicateEndpoint(_) => None,
            InvalidNodeError::DuplicateIdentity(_) => None,
            InvalidNodeError::EmptyEndpoint => None,
            InvalidNodeError::EmptyIdentity => None,
            InvalidNodeError::EmptyDisplayName => None,
            InvalidNodeError::EmptyKey => None,
            InvalidNodeError::InvalidIdentity(..) => None,
            InvalidNodeError::MissingEndpoints => None,
            InvalidNodeError::MissingKeys => None,
        }
    }
}

impl fmt::Display for InvalidNodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            InvalidNodeError::DuplicateEndpoint(endpoint) => {
                write!(f, "another node with endpoint {} exists", endpoint)
            }
            InvalidNodeError::DuplicateIdentity(identity) => {
                write!(f, "another node with identity {} exists", identity)
            }
            InvalidNodeError::EmptyEndpoint => write!(f, "node endpoint cannot be empty"),
            InvalidNodeError::EmptyIdentity => write!(f, "node must have non-empty identity"),
            InvalidNodeError::EmptyDisplayName => {
                write!(f, "node must have non-empty display_name")
            }
            InvalidNodeError::EmptyKey => write!(f, "node key cannot be empty"),
            InvalidNodeError::InvalidIdentity(identity, msg) => {
                write!(f, "identity {} is invalid: {}", identity, msg)
            }
            InvalidNodeError::MissingEndpoints => write!(f, "node must have one or more endpoints"),
            InvalidNodeError::MissingKeys => write!(f, "node must have one or more keys"),
        }
    }
}

#[cfg(feature = "diesel")]
impl From<diesel::result::Error> for RegistryError {
    fn from(err: diesel::result::Error) -> Self {
        match err {
            diesel::result::Error::DatabaseError(db_err_kind, _) => match db_err_kind {
                diesel::result::DatabaseErrorKind::UniqueViolation => {
                    RegistryError::ConstraintViolationError(
                        ConstraintViolationError::from_source_with_violation_type(
                            ConstraintViolationType::Unique,
                            Box::new(err),
                        ),
                    )
                }
                diesel::result::DatabaseErrorKind::ForeignKeyViolation => {
                    RegistryError::ConstraintViolationError(
                        ConstraintViolationError::from_source_with_violation_type(
                            ConstraintViolationType::ForeignKey,
                            Box::new(err),
                        ),
                    )
                }
                _ => RegistryError::InternalError(InternalError::from_source(Box::new(err))),
            },
            _ => RegistryError::InternalError(InternalError::from_source(Box::new(err))),
        }
    }
}
