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

//! Errors that can occur when dealing with identities

use std::error::Error;
use std::fmt;

/// An error that can occur when parsing an `Authorization`
#[derive(Debug)]
pub struct AuthorizationParseError {
    message: String,
}

impl AuthorizationParseError {
    /// Creates a new error
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl fmt::Display for AuthorizationParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for AuthorizationParseError {}
