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

//! Errors that can occur in a service processor

use std::error::Error;
use std::io::Error as IoError;

#[derive(Debug)]
pub enum ServiceProcessorError {
    /// Returned if an error is detected adding a new service
    AddServiceError(String),
    /// Returned if an error is detected while processing requests
    ProcessError(String, Box<dyn Error + Send>),
    /// Returned if an IO error is detected while processing requests
    IoError(IoError),
    /// Returned if an error is detected when trying to shutdown
    ShutdownError(String),
}

impl Error for ServiceProcessorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ServiceProcessorError::AddServiceError(_) => None,
            ServiceProcessorError::ProcessError(_, err) => Some(&**err),
            ServiceProcessorError::IoError(err) => Some(err),
            ServiceProcessorError::ShutdownError(_) => None,
        }
    }
}

impl std::fmt::Display for ServiceProcessorError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            ServiceProcessorError::AddServiceError(ref err) => {
                write!(f, "service cannot be added: {}", err)
            }
            ServiceProcessorError::ProcessError(ref ctx, ref err) => {
                write!(f, "error processing message: {} ({})", ctx, err)
            }
            ServiceProcessorError::IoError(ref err) => {
                write!(f, "io error processing message {}", err)
            }
            ServiceProcessorError::ShutdownError(ref err) => {
                write!(f, "error shutting down: {}", err)
            }
        }
    }
}

impl From<IoError> for ServiceProcessorError {
    fn from(error: IoError) -> Self {
        ServiceProcessorError::IoError(error)
    }
}
