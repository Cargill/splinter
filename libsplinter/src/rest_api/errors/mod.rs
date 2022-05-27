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

mod rest_api_server_error;

use std::error::Error;
use std::fmt;

pub use rest_api_server_error::RestApiServerError;

#[derive(Debug)]
pub enum RequestError {
    MissingHeader(String),
    InvalidHeaderValue(String),
}

impl Error for RequestError {}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RequestError::MissingHeader(msg) => f.write_str(msg),
            RequestError::InvalidHeaderValue(msg) => f.write_str(msg),
        }
    }
}
