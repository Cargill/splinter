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

//! Types for errors that can be raised while using the YAML admin service store

use std::error::Error;
use std::fmt;

/// Represents YamlAdminStoreError errors
#[derive(Debug)]
pub enum YamlAdminStoreError {
    /// A general error occurred in the node registry
    GeneralError {
        context: String,
        source: Option<Box<dyn Error + Send>>,
    },
}

impl YamlAdminStoreError {
    /// Create a new general error with just a context string (no source error).
    pub fn general_error(context: &str) -> Self {
        YamlAdminStoreError::GeneralError {
            context: context.into(),
            source: None,
        }
    }

    /// Create a new general error with a context string and a source error.
    pub fn general_error_with_source(context: &str, err: Box<dyn Error + Send>) -> Self {
        YamlAdminStoreError::GeneralError {
            context: context.into(),
            source: Some(err),
        }
    }
}

impl Error for YamlAdminStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            YamlAdminStoreError::GeneralError { source, .. } => {
                if let Some(ref err) = source {
                    Some(&**err)
                } else {
                    None
                }
            }
        }
    }
}

impl fmt::Display for YamlAdminStoreError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            YamlAdminStoreError::GeneralError { context, source } => {
                if let Some(ref err) = source {
                    write!(f, "{}: {}", context, err)
                } else {
                    f.write_str(&context)
                }
            }
        }
    }
}
