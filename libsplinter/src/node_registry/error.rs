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

#[derive(Debug)]
pub enum NodeRegistryError {
    InvalidNode(InvalidNodeError),
    GeneralError {
        context: String,
        source: Option<Box<dyn Error + Send>>,
    },
}

impl NodeRegistryError {
    pub fn general_error(context: &str) -> Self {
        NodeRegistryError::GeneralError {
            context: context.into(),
            source: None,
        }
    }

    pub fn general_error_with_source(context: &str, err: Box<dyn Error + Send>) -> Self {
        NodeRegistryError::GeneralError {
            context: context.into(),
            source: Some(err),
        }
    }
}

impl Error for NodeRegistryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            NodeRegistryError::InvalidNode(err) => Some(err),
            NodeRegistryError::GeneralError { source, .. } => {
                if let Some(ref err) = source {
                    Some(&**err)
                } else {
                    None
                }
            }
        }
    }
}

impl fmt::Display for NodeRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            NodeRegistryError::InvalidNode(err) => write!(f, "Invalid node detected: {}", err),
            NodeRegistryError::GeneralError { context, source } => {
                if let Some(ref err) = source {
                    write!(f, "{}: {}", context, err)
                } else {
                    f.write_str(&context)
                }
            }
        }
    }
}

impl From<InvalidNodeError> for NodeRegistryError {
    fn from(err: InvalidNodeError) -> Self {
        NodeRegistryError::InvalidNode(err)
    }
}

#[derive(Debug)]
pub enum InvalidNodeError {
    DuplicateEndpoint(String),
    DuplicateIdentity(String),
    EmptyEndpoint,
    EmptyIdentity,
    EmptyDisplayName,
    InvalidIdentity(String, String), // (identity, message)
    MissingEndpoints,
}

impl Error for InvalidNodeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            InvalidNodeError::DuplicateEndpoint(_) => None,
            InvalidNodeError::DuplicateIdentity(_) => None,
            InvalidNodeError::EmptyEndpoint => None,
            InvalidNodeError::EmptyIdentity => None,
            InvalidNodeError::EmptyDisplayName => None,
            InvalidNodeError::InvalidIdentity(..) => None,
            InvalidNodeError::MissingEndpoints => None,
        }
    }
}

impl fmt::Display for InvalidNodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            InvalidNodeError::DuplicateEndpoint(endpoint) => {
                write!(f, "another node with endpoint {} exists", endpoint)
            }
            InvalidNodeError::DuplicateIdentity(identity) => {
                write!(f, "another node with identity {} exists", identity)
            }
            InvalidNodeError::EmptyEndpoint => write!(f, "node endpoint cannot be empty"),
            InvalidNodeError::EmptyIdentity => write!(f, "node must have non-empty identity"),
            InvalidNodeError::EmptyDisplayName => {
                write!(f, "node must have non-empty display_name")
            }
            InvalidNodeError::InvalidIdentity(identity, msg) => {
                write!(f, "identity {} is invalid: {}", identity, msg)
            }
            InvalidNodeError::MissingEndpoints => write!(f, "node must have one or more endpoints"),
        }
    }
}
