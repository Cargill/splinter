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

use crate::error::InternalError;

#[derive(Debug)]
pub enum NextEventError {
    Disconnected,
    InternalError(InternalError),
}

impl fmt::Display for NextEventError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            NextEventError::Disconnected => f.write_str("Disconnected"),
            NextEventError::InternalError(e) => f.write_str(&e.to_string()),
        }
    }
}

impl Error for NextEventError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            NextEventError::Disconnected => None,
            NextEventError::InternalError(ref e) => Some(&*e),
        }
    }
}

#[derive(Debug)]
pub enum WaitForError {
    TimeoutError,
    NextEventError(NextEventError),
}

impl fmt::Display for WaitForError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WaitForError::TimeoutError => f.write_str("Timeout"),
            WaitForError::NextEventError(e) => f.write_str(&e.to_string()),
        }
    }
}

impl Error for WaitForError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            WaitForError::TimeoutError => None,
            WaitForError::NextEventError(ref e) => Some(&*e),
        }
    }
}
