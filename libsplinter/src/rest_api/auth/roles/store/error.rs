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

use crate::error::{ConstraintViolationError, InternalError, InvalidStateError};

#[derive(Debug)]
pub enum RoleBasedAuthorizationStoreError {
    InternalError(InternalError),
    InvalidState(InvalidStateError),
    ConstraintViolation(ConstraintViolationError),
}

impl fmt::Display for RoleBasedAuthorizationStoreError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RoleBasedAuthorizationStoreError::InternalError(err) => err.fmt(f),
            RoleBasedAuthorizationStoreError::InvalidState(err) => err.fmt(f),
            RoleBasedAuthorizationStoreError::ConstraintViolation(err) => err.fmt(f),
        }
    }
}

impl Error for RoleBasedAuthorizationStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RoleBasedAuthorizationStoreError::InternalError(err) => Some(err),
            RoleBasedAuthorizationStoreError::InvalidState(err) => Some(err),
            RoleBasedAuthorizationStoreError::ConstraintViolation(err) => Some(err),
        }
    }
}

impl From<InternalError> for RoleBasedAuthorizationStoreError {
    fn from(err: InternalError) -> Self {
        RoleBasedAuthorizationStoreError::InternalError(err)
    }
}

impl From<InvalidStateError> for RoleBasedAuthorizationStoreError {
    fn from(err: InvalidStateError) -> Self {
        RoleBasedAuthorizationStoreError::InvalidState(err)
    }
}

impl From<ConstraintViolationError> for RoleBasedAuthorizationStoreError {
    fn from(err: ConstraintViolationError) -> Self {
        RoleBasedAuthorizationStoreError::ConstraintViolation(err)
    }
}
