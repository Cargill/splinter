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

//! Traits that abstract specific functionality of a connection group.
//!
//! A component that acts as a connection group peforms the following operations
//!   - add: Adds a connection to the underlying connection group
//!   - remove: Removes a connection from the connection group
//!   - send: Send a message over a specific connection
//!   - recv: Receive a message from any ready connection in the connection group
//!
//! The following Matrix traits abstract out these different operations. For example,
//! a networking component may only be interested in send and receiving messages. That component
//! does not need to be given the methods to add or remove connection, only send and recv.
//!
//! Using these traits will also allow for replacing the backing connection group in the future.

use std::error::Error;
use std::fmt;
use std::time::Duration;

use crate::transport::Connection;

/// Wrapper around a payload to include connection ID
#[derive(Debug, Default, PartialEq)]
pub struct Envelope {
    id: String,
    payload: Vec<u8>,
}

impl Envelope {
    /// Creates a new `Envelope` that will be sent over a connection
    pub fn new(id: String, payload: Vec<u8>) -> Self {
        Envelope { id, payload }
    }

    /// Returns the connection ID of the recipient of the `Envelope`
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the bytes of the payload
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Returns the payload from the message while consuming the the `Envelope`
    pub fn take_payload(self) -> Vec<u8> {
        self.payload
    }
}

/// `MatrixLifeCycle` trait abstracts out adding and removing connections to a
/// connection group without requiring knowledge about sending or receiving messges.
pub trait MatrixLifeCycle: Clone + Send {
    /// Adds a connection to the connection group with a connection ID
    ///
    /// # Arguments
    ///
    /// * `connection` - A boxed connection that will be passed to the connection group.
    /// * `id` - a unique connection ID that will be used to send messages over a connection.
    ///
    /// If the add failed, a `MatrixAddError` will be returned.
    fn add(&self, connection: Box<dyn Connection>, id: String) -> Result<usize, MatrixAddError>;

    /// Removes a connection from the connection group with a connection ID.
    ///
    /// # Arguments
    ///
    /// * `id` - the unique connection ID for the connection that should be removed
    ///
    /// If the remove failed, a `MatrixRemoveError` will be returned.
    fn remove(&self, id: &str) -> Result<Box<dyn Connection>, MatrixRemoveError>;
}

/// `MatrixSender` trait abstracts out sending messages through a connection group without
/// requiring knowledge about adding and removing connections.
pub trait MatrixSender: Clone + Send {
    /// Sends a message over the specified connection.
    ///
    /// # Arguments
    ///
    /// * `id` - the unique connection ID the message should be sent over
    /// * `message` - the bytes of the message
    ///
    /// If the send failed, a `MatrixSendError` will be returned.
    fn send(&self, id: String, message: Vec<u8>) -> Result<(), MatrixSendError>;
}

/// `MatrixReceiver` trait abstracts out receiving messages from a connection group without
/// requiring knowledge about adding and removing connections.
pub trait MatrixReceiver: Clone + Send {
    /// Attempts to receive a message from the connection group. This function will block until
    /// there is a `Envelope` to receive. The message will come from the first ready connection
    /// detected.
    ///
    /// If successful, returns an `Envelope` containing the payload and the connection ID of the
    /// the sender. Otherwise, returns a `MatrixRecvError`.
    fn recv(&self) -> Result<Envelope, MatrixRecvError>;

    /// Attempts to receive a message from the connection group. This function will block until
    /// there is a `Envelope` to receive or the timeout expires
    ///
    /// # Arguments
    ///
    /// * `timeout` - a Duration for the amount of time the function should block waiting on an
    ///     envelope to arrive.
    ///
    /// If successful, returns an `Envelope` containing the payload and the connection ID of the
    /// the sender. Otherwise, returns a `MatrixRecvTimeoutError`.
    fn recv_timeout(&self, timeout: Duration) -> Result<Envelope, MatrixRecvTimeoutError>;
}

/// `MatrixShutdown` trait abstracts out shutting down a connection group without requiring
/// knowledge of the other connection group operations.
pub trait MatrixShutdown: Clone + Send {
    /// Notifies the underlying connection group to shutdown
    fn shutdown(&self);
}

#[derive(Debug)]
pub struct MatrixAddError {
    pub context: String,
    pub source: Option<Box<dyn Error + Send>>,
}

impl MatrixAddError {
    pub fn new(context: String, source: Option<Box<dyn Error + Send>>) -> Self {
        Self { context, source }
    }
}

impl Error for MatrixAddError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if let Some(ref err) = self.source {
            Some(&**err)
        } else {
            None
        }
    }
}

impl fmt::Display for MatrixAddError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref err) = self.source {
            write!(f, "{}: {}", self.context, err)
        } else {
            f.write_str(&self.context)
        }
    }
}

#[derive(Debug)]
pub struct MatrixRemoveError {
    pub context: String,
    pub source: Option<Box<dyn Error + Send>>,
}

impl MatrixRemoveError {
    pub fn new(context: String, source: Option<Box<dyn Error + Send>>) -> Self {
        Self { context, source }
    }
}

impl Error for MatrixRemoveError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if let Some(ref err) = self.source {
            Some(&**err)
        } else {
            None
        }
    }
}

impl fmt::Display for MatrixRemoveError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref err) = self.source {
            write!(f, "{}: {}", self.context, err)
        } else {
            f.write_str(&self.context)
        }
    }
}

#[derive(Debug)]
pub struct MatrixSendError {
    pub context: String,
    pub source: Option<Box<dyn Error + Send>>,
}

impl MatrixSendError {
    pub fn new(context: String, source: Option<Box<dyn Error + Send>>) -> Self {
        Self { context, source }
    }
}

impl Error for MatrixSendError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if let Some(ref err) = self.source {
            Some(&**err)
        } else {
            None
        }
    }
}

impl fmt::Display for MatrixSendError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref err) = self.source {
            write!(f, "{}: {}", self.context, err)
        } else {
            f.write_str(&self.context)
        }
    }
}

#[derive(Debug)]
pub enum MatrixRecvError {
    Disconnected,
    InternalError {
        context: String,
        source: Option<Box<dyn Error + Send>>,
    },
    Shutdown,
}

impl MatrixRecvError {
    pub fn new_internal_error(context: String, source: Option<Box<dyn Error + Send>>) -> Self {
        MatrixRecvError::InternalError { context, source }
    }
}

impl Error for MatrixRecvError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            MatrixRecvError::Disconnected => None,
            MatrixRecvError::InternalError { source, .. } => {
                if let Some(ref err) = source {
                    Some(&**err)
                } else {
                    None
                }
            }
            MatrixRecvError::Shutdown => None,
        }
    }
}

impl fmt::Display for MatrixRecvError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MatrixRecvError::Disconnected => {
                f.write_str("Unable to receive: channel has disconnected")
            }
            MatrixRecvError::InternalError { context, source } => {
                if let Some(ref err) = source {
                    write!(f, "{}: {}", context, err)
                } else {
                    f.write_str(&context)
                }
            }
            MatrixRecvError::Shutdown => f.write_str("Unable to receive: matrix has shutdown"),
        }
    }
}

#[derive(Debug)]
pub enum MatrixRecvTimeoutError {
    Timeout,
    Disconnected,
    InternalError {
        context: String,
        source: Option<Box<dyn Error + Send>>,
    },
    Shutdown,
}

impl MatrixRecvTimeoutError {
    pub fn new_internal_error(context: String, source: Option<Box<dyn Error + Send>>) -> Self {
        MatrixRecvTimeoutError::InternalError { context, source }
    }
}

impl Error for MatrixRecvTimeoutError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            MatrixRecvTimeoutError::Timeout => None,
            MatrixRecvTimeoutError::Disconnected => None,
            MatrixRecvTimeoutError::InternalError { source, .. } => {
                if let Some(ref err) = source {
                    Some(&**err)
                } else {
                    None
                }
            }
            MatrixRecvTimeoutError::Shutdown => None,
        }
    }
}

impl std::fmt::Display for MatrixRecvTimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            MatrixRecvTimeoutError::Timeout => f.write_str("Unable to receive: Timeout"),
            MatrixRecvTimeoutError::Disconnected => {
                f.write_str("Unable to receive: channel has disconnected")
            }
            MatrixRecvTimeoutError::InternalError { context, source } => {
                if let Some(ref err) = source {
                    write!(f, "{}: {}", context, err)
                } else {
                    f.write_str(&context)
                }
            }
            MatrixRecvTimeoutError::Shutdown => {
                f.write_str("Unable to receive: matrix has shutdown")
            }
        }
    }
}
