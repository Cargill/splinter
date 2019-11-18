// Copyright 2019 Cargill Incorporated
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

#[derive(Debug)]
pub enum ModelConversionError {
    DeserializationError(String),
    SerializationError(String),
    InvalidTypeError(String),
}

impl Error for ModelConversionError {
    fn description(&self) -> &str {
        match *self {
            ModelConversionError::DeserializationError(ref msg) => msg,
            ModelConversionError::SerializationError(ref msg) => msg,
            ModelConversionError::InvalidTypeError(ref msg) => msg,
        }
    }

    fn cause(&self) -> Option<&dyn Error> {
        match *self {
            ModelConversionError::DeserializationError(_) => None,
            ModelConversionError::SerializationError(_) => None,
            ModelConversionError::InvalidTypeError(_) => None,
        }
    }
}

impl std::fmt::Display for ModelConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            ModelConversionError::DeserializationError(ref s) => {
                write!(f, "unable to deserialize during model conversion: {}", s)
            }
            ModelConversionError::SerializationError(ref s) => {
                write!(f, "unable to serialize during model conversion: {}", s)
            }
            ModelConversionError::InvalidTypeError(ref s) => {
                write!(f, "invalid type encountered during model conversion: {}", s)
            }
        }
    }
}
