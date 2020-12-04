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
    /// Represents when cab operation cannot be completed because the state of the underlying
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
