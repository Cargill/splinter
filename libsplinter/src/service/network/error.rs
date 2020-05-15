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
pub struct ServiceConnectionAgentError(pub String);

impl Error for ServiceConnectionAgentError {}

impl fmt::Display for ServiceConnectionAgentError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug)]
pub struct ServiceConnectionError(pub String);

impl Error for ServiceConnectionError {}

impl fmt::Display for ServiceConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Errors that may occur on registration.
#[derive(Debug)]
pub enum ServiceAddInstanceError {
    /// The service is not allowed to register for the given circuit on this node.
    NotAllowed,
    /// The service is already registered.
    AlreadyRegistered,
    /// The service does not belong to the specified circuit.
    NotInCircuit,
    /// The specified circuit does not exist.
    CircuitDoesNotExist,
    /// An internal error has occurred while processing the service registration.
    InternalError {
        context: String,
        source: Option<Box<dyn std::error::Error + Send>>,
    },
}

impl Error for ServiceAddInstanceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ServiceAddInstanceError::InternalError {
                source: Some(ref err),
                ..
            } => Some(&**err),
            _ => None,
        }
    }
}

impl fmt::Display for ServiceAddInstanceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ServiceAddInstanceError::NotAllowed => f.write_str("service not allowed on this node"),
            ServiceAddInstanceError::AlreadyRegistered => f.write_str("service already registered"),
            ServiceAddInstanceError::NotInCircuit => f.write_str("service is not in the circuit"),
            ServiceAddInstanceError::CircuitDoesNotExist => f.write_str("circuit does not exist"),
            ServiceAddInstanceError::InternalError {
                context,
                source: Some(ref err),
            } => write!(f, "{}: {}", context, err),
            ServiceAddInstanceError::InternalError {
                context,
                source: None,
            } => f.write_str(&context),
        }
    }
}

/// Errors that may occur on deregistration.
#[derive(Debug)]
pub enum ServiceRemoveInstanceError {
    /// The service is not currently registered with this node.
    NotRegistered,
    /// The service does not belong to the specified circuit.
    NotInCircuit,
    /// The specified circuit does not exist.
    CircuitDoesNotExist,
    /// An internal error has occurred while processing the service deregistration.
    InternalError {
        context: String,
        source: Option<Box<dyn std::error::Error + Send>>,
    },
}

impl Error for ServiceRemoveInstanceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ServiceRemoveInstanceError::InternalError {
                source: Some(ref err),
                ..
            } => Some(&**err),
            _ => None,
        }
    }
}

impl fmt::Display for ServiceRemoveInstanceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ServiceRemoveInstanceError::NotRegistered => f.write_str("service is not registered"),
            ServiceRemoveInstanceError::NotInCircuit => {
                f.write_str("service is not in the circuit")
            }
            ServiceRemoveInstanceError::CircuitDoesNotExist => {
                f.write_str("circuit does not exist")
            }
            ServiceRemoveInstanceError::InternalError {
                context,
                source: Some(ref err),
            } => write!(f, "{}: {}", context, err),
            ServiceRemoveInstanceError::InternalError {
                context,
                source: None,
            } => f.write_str(&context),
        }
    }
}

#[derive(Debug)]
pub enum ServiceForwardingError {
    /// The sending service is not in the circuit.
    SenderNotInCircuit,

    /// The receiving service is not in the circuit.
    RecipientNotInCircuit,

    /// The sending service is not registered.
    SenderNotRegistered,

    /// The receiving service is not registered.
    RecipientNotRegistered,

    /// The specified circuit does not exist.
    CircuitDoesNotExist,

    /// An internal error has occurred while forwarding the message.
    InternalError {
        context: String,
        source: Option<Box<dyn Error + Send>>,
    },
}

impl Error for ServiceForwardingError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ServiceForwardingError::InternalError {
                source: Some(ref err),
                ..
            } => Some(&**err),
            _ => None,
        }
    }
}

impl fmt::Display for ServiceForwardingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ServiceForwardingError::CircuitDoesNotExist => f.write_str("circuit does not exist"),
            ServiceForwardingError::SenderNotInCircuit => {
                f.write_str("sending service is not in the circuit")
            }
            ServiceForwardingError::RecipientNotInCircuit => {
                f.write_str("receiving service is not in the circuit")
            }
            ServiceForwardingError::SenderNotRegistered => {
                f.write_str("sending service is not registered")
            }
            ServiceForwardingError::RecipientNotRegistered => {
                f.write_str("receiving service is not registered")
            }
            ServiceForwardingError::InternalError {
                context,
                source: Some(ref err),
            } => write!(f, "{}: {}", context, err),
            ServiceForwardingError::InternalError {
                context,
                source: None,
            } => f.write_str(&context),
        }
    }
}
