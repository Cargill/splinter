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

//! Errors for the OAuthUserSessionStore.

use std::error::Error;
use std::fmt;

#[cfg(any(feature = "biome-oauth-user-store-postgres", feature = "sqlite"))]
use crate::error::ConstraintViolationType;
use crate::error::{
    ConstraintViolationError, InternalError, InvalidArgumentError, InvalidStateError,
};

/// Errors that may occur during [OAuthUserSessionStore] operations.
#[derive(Debug)]
pub enum OAuthUserSessionStoreError {
    ConstraintViolation(ConstraintViolationError),
    Internal(InternalError),
    InvalidArgument(InvalidArgumentError),
    InvalidState(InvalidStateError),
}

impl Error for OAuthUserSessionStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            OAuthUserSessionStoreError::ConstraintViolation(err) => err.source(),
            OAuthUserSessionStoreError::Internal(err) => err.source(),
            OAuthUserSessionStoreError::InvalidArgument(err) => err.source(),
            OAuthUserSessionStoreError::InvalidState(err) => err.source(),
        }
    }
}

impl fmt::Display for OAuthUserSessionStoreError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            OAuthUserSessionStoreError::ConstraintViolation(err) => f.write_str(&err.to_string()),
            OAuthUserSessionStoreError::Internal(err) => f.write_str(&err.to_string()),
            OAuthUserSessionStoreError::InvalidArgument(err) => f.write_str(&err.to_string()),
            OAuthUserSessionStoreError::InvalidState(err) => f.write_str(&err.to_string()),
        }
    }
}

#[cfg(any(feature = "biome-oauth-user-store-postgres", feature = "sqlite"))]
impl From<diesel::r2d2::PoolError> for OAuthUserSessionStoreError {
    fn from(err: diesel::r2d2::PoolError) -> Self {
        OAuthUserSessionStoreError::Internal(InternalError::from_source(Box::new(err)))
    }
}

#[cfg(any(feature = "biome-oauth-user-store-postgres", feature = "sqlite"))]
impl From<diesel::result::Error> for OAuthUserSessionStoreError {
    fn from(err: diesel::result::Error) -> Self {
        match err {
            diesel::result::Error::DatabaseError(ref kind, _) => match kind {
                diesel::result::DatabaseErrorKind::UniqueViolation => {
                    OAuthUserSessionStoreError::ConstraintViolation(
                        ConstraintViolationError::from_source_with_violation_type(
                            ConstraintViolationType::Unique,
                            Box::new(err),
                        ),
                    )
                }
                diesel::result::DatabaseErrorKind::ForeignKeyViolation => {
                    OAuthUserSessionStoreError::ConstraintViolation(
                        ConstraintViolationError::from_source_with_violation_type(
                            ConstraintViolationType::ForeignKey,
                            Box::new(err),
                        ),
                    )
                }
                _ => {
                    OAuthUserSessionStoreError::Internal(InternalError::from_source(Box::new(err)))
                }
            },
            _ => OAuthUserSessionStoreError::Internal(InternalError::from_source(Box::new(err))),
        }
    }
}
