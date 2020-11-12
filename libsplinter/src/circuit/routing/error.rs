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

//! Types for errors that can be raised by `RoutingTableReader` and `RoutingTableWriter` traits

use std::error::Error;
use std::fmt;

use crate::error::{InternalError, InvalidStateError};

#[derive(Debug)]
pub enum RoutingTableReaderError {
    /// Represents errors internal to the function.
    InternalError(InternalError),
    /// Represents when cab operation cannot be completed because the state of the underlying
    /// struct is inconsistent.
    InvalidStateError(InvalidStateError),
}

impl Error for RoutingTableReaderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RoutingTableReaderError::InternalError(err) => Some(err),
            RoutingTableReaderError::InvalidStateError(err) => Some(err),
        }
    }
}

impl fmt::Display for RoutingTableReaderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RoutingTableReaderError::InternalError(err) => write!(f, "{}", err),
            RoutingTableReaderError::InvalidStateError(err) => write!(f, "{}", err),
        }
    }
}

/// Errors that could be raised when adding a service
#[derive(Debug, PartialEq)]
pub struct AddServiceError(pub String);

impl Error for AddServiceError {}

impl fmt::Display for AddServiceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Received internal error: {}", self.0)
    }
}

/// Errors that could be raised when removing a service
#[derive(Debug, PartialEq)]
pub struct RemoveServiceError(pub String);

impl Error for RemoveServiceError {}

impl fmt::Display for RemoveServiceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Received internal error: {}", self.0)
    }
}

/// Errors that could be raised when adding a circuit
#[derive(Debug, PartialEq)]
pub struct AddCircuitError(pub String);

impl Error for AddCircuitError {}

impl fmt::Display for AddCircuitError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Received internal error: {}", self.0)
    }
}

/// Errors that could be raised when adding circuits
#[derive(Debug, PartialEq)]
pub struct AddCircuitsError(pub String);

impl Error for AddCircuitsError {}

impl fmt::Display for AddCircuitsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Received internal error: {}", self.0)
    }
}

/// Errors that could be raised when removing a circuit
#[derive(Debug, PartialEq)]
pub struct RemoveCircuitError(pub String);

impl Error for RemoveCircuitError {}

impl fmt::Display for RemoveCircuitError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Received internal error: {}", self.0)
    }
}

/// Errors that could be raised when adding a node
#[derive(Debug, PartialEq)]
pub struct AddNodeError(pub String);

impl Error for AddNodeError {}

impl fmt::Display for AddNodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Received internal error: {}", self.0)
    }
}

/// Errors that could be raised when adding nodes
#[derive(Debug, PartialEq)]
pub struct AddNodesError(pub String);

impl Error for AddNodesError {}

impl fmt::Display for AddNodesError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Received internal error: {}", self.0)
    }
}

/// Errors that could be raised when removing a node
#[derive(Debug, PartialEq)]
pub struct RemoveNodeError(pub String);

impl Error for RemoveNodeError {}

impl fmt::Display for RemoveNodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Received internal error: {}", self.0)
    }
}
