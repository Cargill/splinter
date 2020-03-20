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

/// Error module for `rest_api`.
#[derive(Debug)]
pub enum RestApiServerError {
    BindError(String),
    StartUpError(String),
    MissingField(String),
    StdError(std::io::Error),
}

impl From<std::io::Error> for RestApiServerError {
    fn from(err: std::io::Error) -> RestApiServerError {
        RestApiServerError::StdError(err)
    }
}

impl Error for RestApiServerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RestApiServerError::BindError(_) => None,
            RestApiServerError::StartUpError(_) => None,
            RestApiServerError::StdError(err) => Some(err),
            RestApiServerError::MissingField(_) => None,
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
        }
    }
}

#[derive(Debug)]
pub struct WebSocketError {
    context: String,
    source: Option<Box<dyn Error>>,
}

impl WebSocketError {
    pub fn new(context: &str) -> Self {
        Self {
            context: context.into(),
            source: None,
        }
    }

    pub fn new_with_source(context: &str, err: Box<dyn Error>) -> Self {
        Self {
            context: context.into(),
            source: Some(err),
        }
    }
}

impl Error for WebSocketError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_ref().map(|err| &**err)
    }
}

impl fmt::Display for WebSocketError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.source {
            Some(ref err) => write!(f, "{}: {}", self.context, err),
            None => f.write_str(&self.context),
        }
    }
}

#[derive(Debug)]
pub(super) enum RequestBuilderError {
    MissingRequiredField(String),
}

impl Error for RequestBuilderError {}

impl fmt::Display for RequestBuilderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RequestBuilderError::MissingRequiredField(field) => {
                write!(f, "Required field is missing: {}", field)
            }
        }
    }
}
