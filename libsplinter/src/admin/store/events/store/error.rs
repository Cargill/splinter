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

//! Types for errors that can be raised while using an admin service event store
use std::fmt;

use crate::admin::store::error::AdminServiceStoreError;
#[cfg(feature = "admin-service-event-store-diesel")]
use crate::error::ConstraintViolationType;
use crate::error::{
    ConstraintViolationError, InternalError, InvalidStateError, ResourceTemporarilyUnavailableError,
};

/// Represents AdminServiceEventStoreError errors
#[derive(Debug)]
pub enum AdminServiceEventStoreError {
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

impl fmt::Display for AdminServiceEventStoreError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AdminServiceEventStoreError::InternalError(err) => write!(f, "{}", err),
            AdminServiceEventStoreError::ConstraintViolationError(err) => write!(f, "{}", err),
            AdminServiceEventStoreError::ResourceTemporarilyUnavailableError(err) => {
                write!(f, "{}", err)
            }
            AdminServiceEventStoreError::InvalidStateError(err) => write!(f, "{}", err),
        }
    }
}

impl From<AdminServiceStoreError> for AdminServiceEventStoreError {
    fn from(err: AdminServiceStoreError) -> Self {
        match err {
            AdminServiceStoreError::InternalError(err) => {
                AdminServiceEventStoreError::InternalError(err)
            }
            AdminServiceStoreError::ConstraintViolationError(err) => {
                AdminServiceEventStoreError::ConstraintViolationError(err)
            }
            AdminServiceStoreError::ResourceTemporarilyUnavailableError(err) => {
                AdminServiceEventStoreError::ResourceTemporarilyUnavailableError(err)
            }
            AdminServiceStoreError::InvalidStateError(err) => {
                AdminServiceEventStoreError::InvalidStateError(err)
            }
        }
    }
}

#[cfg(feature = "admin-service-event-store-diesel")]
impl From<diesel::r2d2::PoolError> for AdminServiceEventStoreError {
    fn from(err: diesel::r2d2::PoolError) -> Self {
        AdminServiceEventStoreError::ResourceTemporarilyUnavailableError(
            ResourceTemporarilyUnavailableError::from_source(Box::new(err)),
        )
    }
}

#[cfg(feature = "admin-service-event-store-diesel")]
impl From<diesel::result::Error> for AdminServiceEventStoreError {
    fn from(err: diesel::result::Error) -> Self {
        match err {
            diesel::result::Error::DatabaseError(db_err_kind, _) => match db_err_kind {
                diesel::result::DatabaseErrorKind::UniqueViolation => {
                    AdminServiceEventStoreError::ConstraintViolationError(
                        ConstraintViolationError::from_source_with_violation_type(
                            ConstraintViolationType::Unique,
                            Box::new(err),
                        ),
                    )
                }
                diesel::result::DatabaseErrorKind::ForeignKeyViolation => {
                    AdminServiceEventStoreError::ConstraintViolationError(
                        ConstraintViolationError::from_source_with_violation_type(
                            ConstraintViolationType::ForeignKey,
                            Box::new(err),
                        ),
                    )
                }
                _ => AdminServiceEventStoreError::InternalError(InternalError::from_source(
                    Box::new(err),
                )),
            },
            _ => AdminServiceEventStoreError::InternalError(InternalError::from_source(Box::new(
                err,
            ))),
        }
    }
}
