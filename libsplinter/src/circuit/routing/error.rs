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

/// Errors that could be raised when requesting a service
#[derive(Debug, PartialEq)]
pub struct FetchServiceError(pub String);

impl Error for FetchServiceError {}

impl fmt::Display for FetchServiceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Received internal error: {}", self.0)
    }
}

/// Errors that could be raised when requesting all services in a circuit
#[derive(Debug, PartialEq)]
pub enum ListServiceError {
    /// Internal error
    InternalError(String),
    CircuitNotFound(String),
}

impl Error for ListServiceError {}

impl fmt::Display for ListServiceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ListServiceError::InternalError(msg) => write!(f, "Received internal error: {}", msg),
            ListServiceError::CircuitNotFound(circuit_id) => {
                write!(f, "Circuit does not exist: {}", circuit_id)
            }
        }
    }
}

/// Errors that could be raised when requesting all nodes
#[derive(Debug, PartialEq)]
pub struct ListNodesError(pub String);

impl Error for ListNodesError {}

impl fmt::Display for ListNodesError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Received internal error: {}", self.0)
    }
}

/// Errors that could be raised when requesting a node
#[derive(Debug, PartialEq)]
pub struct FetchNodeError(pub String);

impl Error for FetchNodeError {}

impl fmt::Display for FetchNodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Received internal error: {}", self.0)
    }
}

/// Errors that could be raised when requesting all circuits
#[derive(Debug, PartialEq)]
pub struct ListCircuitsError(pub String);

impl Error for ListCircuitsError {}

impl fmt::Display for ListCircuitsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Received internal error: {}", self.0)
    }
}

/// Errors that could be raised when requesting a circuit
#[derive(Debug, PartialEq)]
pub struct FetchCircuitError(pub String);

impl Error for FetchCircuitError {}

impl fmt::Display for FetchCircuitError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Received internal error: {}", self.0)
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
