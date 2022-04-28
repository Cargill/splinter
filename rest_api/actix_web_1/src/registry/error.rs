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

//! Contains an error tht can be sent from

use std::error::Error;
use std::fmt;

use splinter::error::InvalidStateError;
use splinter::registry::RegistryError;

/// Represents errors that occur with node registry operations while using the REST API
#[derive(Debug)]
pub enum RegistryRestApiError {
    /// Represents errors internal to the function
    InternalError(String),
    /// Represent invalid node errors
    InvalidStateError(InvalidStateError),
}

impl Error for RegistryRestApiError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RegistryRestApiError::InternalError(_) => None,
            RegistryRestApiError::InvalidStateError(err) => Some(err),
        }
    }
}

impl fmt::Display for RegistryRestApiError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RegistryRestApiError::InternalError(msg) => write!(f, "{}", msg),
            RegistryRestApiError::InvalidStateError(err) => write!(f, "{}", err),
        }
    }
}

impl From<RegistryError> for RegistryRestApiError {
    fn from(err: RegistryError) -> Self {
        match err {
            RegistryError::InvalidStateError(err) => RegistryRestApiError::InvalidStateError(err),
            _ => RegistryRestApiError::InternalError(err.to_string()),
        }
    }
}
