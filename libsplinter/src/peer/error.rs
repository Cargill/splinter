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

//! Types for errors that can be raised by the PeerManager.

use std::{error, fmt};

/// Errors that could be raised by the `PeerManager`
#[derive(Debug, PartialEq)]
pub enum PeerManagerError {
    /// `PeerManager` start up failed
    StartUpError(String),
    /// A message failed to send
    SendMessageError(String),
}

impl error::Error for PeerManagerError {}

impl fmt::Display for PeerManagerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PeerManagerError::StartUpError(msg) => write!(f, "{}", msg),
            PeerManagerError::SendMessageError(msg) => write!(f, "{}", msg),
        }
    }
}

/// Errors that could be raised when requesting a peer is added
#[derive(Debug, PartialEq)]
pub enum PeerRefAddError {
    /// Internal `PeerManager` error
    InternalError(String),
    /// Unable to receive response
    ReceiveError(String),
    /// Unable to add requested peer
    AddError(String),
}

impl error::Error for PeerRefAddError {}

impl fmt::Display for PeerRefAddError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PeerRefAddError::InternalError(msg) => write!(f, "Received internal error: {}", msg),
            PeerRefAddError::ReceiveError(msg) => {
                write!(f, "Unable to receive response from PeerManager: {}", msg)
            }
            PeerRefAddError::AddError(msg) => write!(f, "Unable to add peer: {}", msg),
        }
    }
}

/// Errors that could be raised when requesting a peer is added without a peer ID
#[derive(Debug, PartialEq)]
pub enum PeerUnknownAddError {
    /// Internal `PeerManager` error
    InternalError(String),
    /// Unable to receive response
    ReceiveError(String),
    /// Unable to add requested peer
    AddError(String),
}

impl error::Error for PeerUnknownAddError {}

impl fmt::Display for PeerUnknownAddError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PeerUnknownAddError::InternalError(msg) => {
                write!(f, "Received internal error: {}", msg)
            }
            PeerUnknownAddError::ReceiveError(msg) => {
                write!(f, "Unable to receive response from PeerManager: {}", msg)
            }
            PeerUnknownAddError::AddError(msg) => {
                write!(f, "Unable to add unidentified peer: {}", msg)
            }
        }
    }
}

/// Errors that could be raised when requesting a peer is removed
#[derive(Debug, PartialEq)]
pub enum PeerRefRemoveError {
    /// Internal `PeerManager` error
    InternalError(String),
    /// Unable to receive response
    ReceiveError(String),
    /// Unable to remove requested peer
    RemoveError(String),
}

impl error::Error for PeerRefRemoveError {}

impl fmt::Display for PeerRefRemoveError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PeerRefRemoveError::InternalError(msg) => write!(f, "Received internal error: {}", msg),
            PeerRefRemoveError::ReceiveError(msg) => {
                write!(f, "Unable to receive response from PeerManager: {}", msg)
            }
            PeerRefRemoveError::RemoveError(msg) => write!(f, "Unable to remove peer: {}", msg),
        }
    }
}

/// Errors that could be raised when requesting a list of peers
#[derive(Debug, PartialEq)]
pub enum PeerListError {
    /// Internal `PeerManager`error
    InternalError(String),
    /// Unable to receive response
    ReceiveError(String),
    /// Unable to get current list of peers
    ListError(String),
}

impl error::Error for PeerListError {}

impl fmt::Display for PeerListError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PeerListError::InternalError(msg) => write!(f, "Received internal error: {}", msg),
            PeerListError::ReceiveError(msg) => {
                write!(f, "Unable to receive response from PeerManager: {}", msg)
            }
            PeerListError::ListError(msg) => write!(f, "Unable to list peers: {}", msg),
        }
    }
}

/// Errors that could be raised when requesting a peer's connection ID
#[derive(Debug, PartialEq)]
pub enum PeerConnectionIdError {
    /// Internal `PeerManager` error
    InternalError(String),
    /// Unable to receive response
    ReceiveError(String),
    /// Unable to get peer's connection ID
    ListError(String),
}

impl error::Error for PeerConnectionIdError {}

impl fmt::Display for PeerConnectionIdError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PeerConnectionIdError::InternalError(msg) => {
                write!(f, "Received internal error: {}", msg)
            }
            PeerConnectionIdError::ReceiveError(msg) => {
                write!(f, "Unable to receive response from PeerManager: {}", msg)
            }
            PeerConnectionIdError::ListError(msg) => {
                write!(f, "Unable to get connection id map: {}", msg)
            }
        }
    }
}

/// Errors raised by trying to update a peer
#[derive(Debug)]
pub struct PeerUpdateError(pub String);

impl error::Error for PeerUpdateError {}

impl fmt::Display for PeerUpdateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Unable to update peer, {}", self.0)
    }
}

/// Errors that could be raised by `PeerInterconnect`
#[derive(Debug, PartialEq)]
pub enum PeerInterconnectError {
    /// `PeerInterconnect` start up failed
    StartUpError(String),
}

impl error::Error for PeerInterconnectError {}

impl fmt::Display for PeerInterconnectError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PeerInterconnectError::StartUpError(msg) => {
                write!(f, "Unable to start peer interconnect: {}", msg)
            }
        }
    }
}

/// Errors that could be raised when looking up a peer
#[derive(Debug)]
pub struct PeerLookupError(pub String);

impl error::Error for PeerLookupError {}

impl fmt::Display for PeerLookupError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}
