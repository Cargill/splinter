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

//! Provides errors relating to testing framework builders

use splinter::admin::client::event::EventType;
use splinter::error::{InternalError, InvalidArgumentError};
use std::error::Error;
use std::fmt;

/// `CircuitBuildError` represents any errors that may arise during the final circuit build step
#[derive(Debug)]
pub enum CircuitBuildError {
    UnexpectedEvent { expected: String, got: EventType },
    Internal(InternalError),
}

impl fmt::Display for CircuitBuildError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CircuitBuildError::UnexpectedEvent { expected, got } => {
                f.write_str(&format!("expected event type: {:?} got: {:?}", expected, got)[..])
            }
            CircuitBuildError::Internal(_) => f.write_str("internal error encountered"),
        }
    }
}

impl Error for CircuitBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match *self {
            CircuitBuildError::UnexpectedEvent { .. } => None,
            CircuitBuildError::Internal(ref e) => Some(e),
        }
    }
}

impl From<InternalError> for CircuitBuildError {
    fn from(err: InternalError) -> Self {
        CircuitBuildError::Internal(err)
    }
}

/// `AddScabbardServiceError` represents errors while adding a scabbard service
#[derive(Debug)]
pub enum AddScabbardServiceError {
    InvalidArgument(InvalidArgumentError),
    Internal(InternalError),
}

impl fmt::Display for AddScabbardServiceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AddScabbardServiceError::InvalidArgument(_) => {
                f.write_str("invalid args provided for scabbard circuit")
            }
            AddScabbardServiceError::Internal(_) => f.write_str("scabbard circuit internal error"),
        }
    }
}

impl Error for AddScabbardServiceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match *self {
            AddScabbardServiceError::InvalidArgument(ref e) => Some(e),
            AddScabbardServiceError::Internal(ref e) => Some(e),
        }
    }
}
