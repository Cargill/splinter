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

use splinter::error::{InternalError, InvalidStateError};
#[cfg(feature = "oauth")]
use splinter::oauth::OAuthClientBuildError;

/// Error module for `rest_api`.
#[derive(Debug)]
pub enum RestApiServerError {
    BindError(String),
    StartUpError(String),
    MissingField(String),
    StdError(std::io::Error),
    InvalidStateError(InvalidStateError),
    InternalError(InternalError),
}

impl From<std::io::Error> for RestApiServerError {
    fn from(err: std::io::Error) -> RestApiServerError {
        RestApiServerError::StdError(err)
    }
}

#[cfg(feature = "oauth")]
impl From<OAuthClientBuildError> for RestApiServerError {
    fn from(err: OAuthClientBuildError) -> Self {
        match err {
            OAuthClientBuildError::InvalidStateError(err) => Self::InvalidStateError(err),
            OAuthClientBuildError::InternalError(err) => Self::InternalError(err),
            _ => Self::InternalError(InternalError::from_source(err.into())),
        }
    }
}

impl Error for RestApiServerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RestApiServerError::BindError(_) => None,
            RestApiServerError::StartUpError(_) => None,
            RestApiServerError::StdError(err) => Some(err),
            RestApiServerError::MissingField(_) => None,
            RestApiServerError::InvalidStateError(err) => Some(err),
            RestApiServerError::InternalError(err) => Some(err),
        }
    }
}

impl fmt::Display for RestApiServerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RestApiServerError::BindError(e) => write!(f, "Bind Error: {}", e),
            RestApiServerError::StartUpError(e) => write!(f, "Start-up Error: {}", e),
            RestApiServerError::StdError(e) => write!(f, "Std Error: {}", e),
            RestApiServerError::MissingField(field) => {
                write!(f, "Missing required field: {}", field)
            }
            RestApiServerError::InvalidStateError(e) => write!(f, "{}", e),
            RestApiServerError::InternalError(e) => write!(f, "{}", e),
        }
    }
}

#[cfg(feature = "https-bind")]
impl From<openssl::error::ErrorStack> for RestApiServerError {
    fn from(err: openssl::error::ErrorStack) -> Self {
        RestApiServerError::InternalError(InternalError::from_source(Box::new(err)))
    }
}
