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

//! Error types and logic for LifecycleStore.

use std::error::Error;
use std::fmt::Display;

#[cfg(feature = "diesel")]
use crate::error::ConstraintViolationType;
use crate::error::{
    ConstraintViolationError, InternalError, InvalidArgumentError, InvalidStateError,
    ResourceTemporarilyUnavailableError,
};

/// Error type for the LifecycleStore trait.
#[derive(Debug)]
pub enum LifecycleStoreError {
    ConstraintViolation(ConstraintViolationError),
    Internal(InternalError),
    InvalidArgument(InvalidArgumentError),
    InvalidState(InvalidStateError),
    ResourceTemporarilyUnavailable(ResourceTemporarilyUnavailableError),
}

impl Display for LifecycleStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LifecycleStoreError::ConstraintViolation(e) => e.fmt(f),
            LifecycleStoreError::Internal(e) => e.fmt(f),
            LifecycleStoreError::InvalidArgument(e) => e.fmt(f),
            LifecycleStoreError::InvalidState(e) => e.fmt(f),
            LifecycleStoreError::ResourceTemporarilyUnavailable(e) => e.fmt(f),
        }
    }
}

impl Error for LifecycleStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            LifecycleStoreError::ConstraintViolation(e) => Some(e),
            LifecycleStoreError::Internal(e) => Some(e),
            LifecycleStoreError::InvalidArgument(e) => Some(e),
            LifecycleStoreError::InvalidState(e) => Some(e),
            LifecycleStoreError::ResourceTemporarilyUnavailable(e) => Some(e),
        }
    }
}

impl From<InternalError> for LifecycleStoreError {
    fn from(err: InternalError) -> Self {
        LifecycleStoreError::Internal(err)
    }
}

impl From<InvalidArgumentError> for LifecycleStoreError {
    fn from(err: InvalidArgumentError) -> Self {
        LifecycleStoreError::InvalidArgument(err)
    }
}

impl From<InvalidStateError> for LifecycleStoreError {
    fn from(err: InvalidStateError) -> Self {
        LifecycleStoreError::InvalidState(err)
    }
}

#[cfg(feature = "diesel")]
impl From<diesel::r2d2::PoolError> for LifecycleStoreError {
    fn from(err: diesel::r2d2::PoolError) -> Self {
        LifecycleStoreError::ResourceTemporarilyUnavailable(
            ResourceTemporarilyUnavailableError::from_source(Box::new(err)),
        )
    }
}

#[cfg(feature = "diesel")]
impl From<diesel::result::Error> for LifecycleStoreError {
    fn from(err: diesel::result::Error) -> Self {
        match err {
            diesel::result::Error::DatabaseError(db_err_kind, _) => match db_err_kind {
                diesel::result::DatabaseErrorKind::UniqueViolation => {
                    LifecycleStoreError::ConstraintViolation(
                        ConstraintViolationError::from_source_with_violation_type(
                            ConstraintViolationType::Unique,
                            Box::new(err),
                        ),
                    )
                }
                diesel::result::DatabaseErrorKind::ForeignKeyViolation => {
                    LifecycleStoreError::ConstraintViolation(
                        ConstraintViolationError::from_source_with_violation_type(
                            ConstraintViolationType::ForeignKey,
                            Box::new(err),
                        ),
                    )
                }
                _ => LifecycleStoreError::Internal(InternalError::from_source(Box::new(err))),
            },
            _ => LifecycleStoreError::Internal(InternalError::from_source(Box::new(err))),
        }
    }
}
