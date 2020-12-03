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

use crate::error::{InternalError, InvalidArgumentError, InvalidStateError};

/// An error that may occur when building an OAuthClient
#[derive(Debug)]
pub enum OAuthClientBuildError {
    InvalidStateError(InvalidStateError),
    InvalidArgumentError(InvalidArgumentError),
    InternalError(InternalError),
}

impl fmt::Display for OAuthClientBuildError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            OAuthClientBuildError::InvalidStateError(err) => f.write_str(&err.to_string()),
            OAuthClientBuildError::InvalidArgumentError(err) => f.write_str(&err.to_string()),
            OAuthClientBuildError::InternalError(err) => f.write_str(&err.to_string()),
        }
    }
}

impl Error for OAuthClientBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            OAuthClientBuildError::InvalidStateError(err) => Some(err),
            OAuthClientBuildError::InvalidArgumentError(err) => Some(err),
            OAuthClientBuildError::InternalError(err) => Some(err),
        }
    }
}

impl From<InvalidStateError> for OAuthClientBuildError {
    fn from(err: InvalidStateError) -> Self {
        OAuthClientBuildError::InvalidStateError(err)
    }
}

impl From<InvalidArgumentError> for OAuthClientBuildError {
    fn from(err: InvalidArgumentError) -> Self {
        OAuthClientBuildError::InvalidArgumentError(err)
    }
}

impl From<InternalError> for OAuthClientBuildError {
    fn from(err: InternalError) -> Self {
        OAuthClientBuildError::InternalError(err)
    }
}
