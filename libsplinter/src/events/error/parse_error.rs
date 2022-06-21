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

#[derive(Debug)]
pub enum ParseError {
    MalformedMessage(Box<dyn Error + Send + Sync + 'static>),
    /// This type provides a sendable alternative to source errors that may not be Sync and Send.
    MalformedReducedToString(String),
}

impl Error for ParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ParseError::MalformedMessage(_) => None,
            ParseError::MalformedReducedToString(_) => None,
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParseError::MalformedMessage(err) => write!(f, "Malformed message {}", err),
            ParseError::MalformedReducedToString(msg) => f.write_str(msg),
        }
    }
}
