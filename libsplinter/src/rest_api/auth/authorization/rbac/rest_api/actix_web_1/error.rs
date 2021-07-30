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

//! Thread-friendly version of the RoleBasedAuthorizationStoreError

use std::error::Error;
use std::fmt;

use crate::error::{ConstraintViolationType, InvalidStateError};
use crate::rest_api::auth::authorization::rbac::store::RoleBasedAuthorizationStoreError;

#[derive(Debug)]
pub(crate) enum SendableRoleBasedAuthorizationStoreError {
    ConstraintViolation(String),
    InternalError(String),
    InvalidState(InvalidStateError),
    NotFound(String),
}

impl Error for SendableRoleBasedAuthorizationStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SendableRoleBasedAuthorizationStoreError::ConstraintViolation(_) => None,
            SendableRoleBasedAuthorizationStoreError::InternalError(_) => None,
            SendableRoleBasedAuthorizationStoreError::InvalidState(err) => err.source(),
            SendableRoleBasedAuthorizationStoreError::NotFound(_) => None,
        }
    }
}

impl fmt::Display for SendableRoleBasedAuthorizationStoreError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SendableRoleBasedAuthorizationStoreError::ConstraintViolation(err) => f.write_str(err),
            SendableRoleBasedAuthorizationStoreError::InternalError(err) => f.write_str(err),
            SendableRoleBasedAuthorizationStoreError::InvalidState(err) => {
                f.write_str(&err.to_string())
            }
            SendableRoleBasedAuthorizationStoreError::NotFound(msg) => f.write_str(msg),
        }
    }
}

impl From<RoleBasedAuthorizationStoreError> for SendableRoleBasedAuthorizationStoreError {
    fn from(err: RoleBasedAuthorizationStoreError) -> Self {
        match err {
            RoleBasedAuthorizationStoreError::ConstraintViolation(err)
                if err.violation_type() == &ConstraintViolationType::NotFound =>
            {
                SendableRoleBasedAuthorizationStoreError::NotFound(err.to_string())
            }
            RoleBasedAuthorizationStoreError::ConstraintViolation(err) => {
                SendableRoleBasedAuthorizationStoreError::ConstraintViolation(err.to_string())
            }
            RoleBasedAuthorizationStoreError::InvalidState(err) => {
                SendableRoleBasedAuthorizationStoreError::InvalidState(err)
            }
            RoleBasedAuthorizationStoreError::InternalError(err) => {
                SendableRoleBasedAuthorizationStoreError::InternalError(err.reduce_to_string())
            }
        }
    }
}
