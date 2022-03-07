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
use std::fmt::Display;

#[derive(Debug)]
#[non_exhaustive]
pub enum ResponseError {
    BadRequest(String),
    NotFound(String),
    InternalError(String, Option<Box<dyn Error>>),
    NotAuthorized,
}

impl std::error::Error for ResponseError {}

impl Display for ResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResponseError::BadRequest(msg) => write!(f, "Bad Request: {}", msg),
            ResponseError::NotFound(url) => write!(f, "Could not find resource for: {}", url),
            ResponseError::InternalError(msg, Some(err)) => {
                write!(f, "Internal Error: {}: {}", msg, err)
            }
            ResponseError::InternalError(msg, None) => write!(f, "Internal Error: {}", msg),
            ResponseError::NotAuthorized => write!(f, "Not Authorized"),
        }
    }
}

impl ResponseError {
    pub fn bad_request<S: Into<String>>(msg: S) -> Self {
        Self::BadRequest(msg.into())
    }

    pub fn not_found<S: Into<String>>(url: S) -> Self {
        Self::NotFound(url.into())
    }

    pub fn internal_error<S: Into<String>>(msg: S, err: Option<Box<dyn Error>>) -> Self {
        Self::InternalError(msg.into(), err)
    }
}
