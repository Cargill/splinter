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

//! Types for errors that can be raised while using an admin service store
use std::error::Error;
use std::fmt;

use crate::error::{
    ConstraintViolationError, ConstraintViolationType, InternalError, InvalidStateError,
    ResourceTemporarilyUnavailableError,
};

/// Represents AdminServiceStore errors
#[derive(Debug)]
pub enum AdminServiceStoreError {
    /// Represents errors internal to the function.
    InternalError(InternalError),
    /// Represents constraint violations on the database's definition
    ConstraintViolationError(ConstraintViolationError),
    /// Represents when the underlying resource is unavailable
    ResourceTemporarilyUnavailableError(ResourceTemporarilyUnavailableError),
    /// Represents when cab operation cannot be completed because the state of the underlying
    /// struct is inconsistent.
    InvalidStateError(InvalidStateError),
}

impl Error for AdminServiceStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AdminServiceStoreError::InternalError(err) => Some(err),
            AdminServiceStoreError::ConstraintViolationError(err) => Some(err),
            AdminServiceStoreError::ResourceTemporarilyUnavailableError(err) => Some(err),
            AdminServiceStoreError::InvalidStateError(err) => Some(err),
        }
    }
}

impl fmt::Display for AdminServiceStoreError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AdminServiceStoreError::InternalError(err) => write!(f, "{}", err),
            AdminServiceStoreError::ConstraintViolationError(err) => write!(f, "{}", err),
            AdminServiceStoreError::ResourceTemporarilyUnavailableError(err) => {
                write!(f, "{}", err)
            }
            AdminServiceStoreError::InvalidStateError(err) => write!(f, "{}", err),
        }
    }
}

#[cfg(feature = "diesel")]
impl From<diesel::r2d2::PoolError> for AdminServiceStoreError {
    fn from(err: diesel::r2d2::PoolError) -> Self {
        AdminServiceStoreError::ResourceTemporarilyUnavailableError(
            ResourceTemporarilyUnavailableError::from_source(Box::new(err)),
        )
    }
}

#[cfg(feature = "diesel")]
impl From<diesel::result::Error> for AdminServiceStoreError {
    fn from(err: diesel::result::Error) -> Self {
        match err {
            diesel::result::Error::DatabaseError(db_err_kind, _) => match db_err_kind {
                diesel::result::DatabaseErrorKind::UniqueViolation => {
                    AdminServiceStoreError::ConstraintViolationError(
                        ConstraintViolationError::from_source_with_violation_type(
                            ConstraintViolationType::Unique,
                            Box::new(err),
                        ),
                    )
                }
                diesel::result::DatabaseErrorKind::ForeignKeyViolation => {
                    AdminServiceStoreError::ConstraintViolationError(
                        ConstraintViolationError::from_source_with_violation_type(
                            ConstraintViolationType::ForeignKey,
                            Box::new(err),
                        ),
                    )
                }
                _ => {
                    AdminServiceStoreError::InternalError(InternalError::from_source(Box::new(err)))
                }
            },
            _ => AdminServiceStoreError::InternalError(InternalError::from_source(Box::new(err))),
        }
    }
}
