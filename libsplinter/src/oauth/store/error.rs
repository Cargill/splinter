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

//! Errors for the InflightOAuthRequestStore.

use std::error::Error;
use std::fmt;

use crate::error::{ConstraintViolationError, InternalError};

/// Errors that may occur during InflightOAuthRequestStore operations.
#[derive(Debug)]
pub enum InflightOAuthRequestStoreError {
    InternalError(InternalError),
    ConstraintViolation(ConstraintViolationError),
}

impl Error for InflightOAuthRequestStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            InflightOAuthRequestStoreError::InternalError(err) => err.source(),
            InflightOAuthRequestStoreError::ConstraintViolation(err) => err.source(),
        }
    }
}

impl fmt::Display for InflightOAuthRequestStoreError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            InflightOAuthRequestStoreError::InternalError(err) => f.write_str(&err.to_string()),
            InflightOAuthRequestStoreError::ConstraintViolation(err) => {
                f.write_str(&err.to_string())
            }
        }
    }
}

impl From<InternalError> for InflightOAuthRequestStoreError {
    fn from(err: InternalError) -> Self {
        InflightOAuthRequestStoreError::InternalError(err)
    }
}
