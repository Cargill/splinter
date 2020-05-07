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

use std::time::Duration;

use super::Connection;

pub use super::error::{
    ConnectionMatrixAddError, ConnectionMatrixRecvError, ConnectionMatrixRecvTimeoutError,
    ConnectionMatrixRemoveError, ConnectionMatrixSendError,
};

/// Wrapper around a payload to include connection ID
#[derive(Debug, Default, PartialEq)]
pub struct ConnectionMatrixEnvelope {
    id: String,
    payload: Vec<u8>,
}

impl ConnectionMatrixEnvelope {
    /// Creates a new `ConnectionMatrixEnvelope` that will be sent over a connection
    pub fn new(id: String, payload: Vec<u8>) -> Self {
        ConnectionMatrixEnvelope { id, payload }
    }

    /// Returns the connection ID of the recipient of the `ConnectionMatrixEnvelope`
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the bytes of the payload
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Returns the payload from the message while consuming the the `ConnectionMatrixEnvelope`
    pub fn take_payload(self) -> Vec<u8> {
        self.payload
    }
}

/// `ConnectionMatrixLifeCycle` trait abstracts out adding and removing connections to a connection
/// group without requiring knowledge about sending or receiving messges.
pub trait ConnectionMatrixLifeCycle: Clone + Send {
    /// Adds a connection to the connection group with a connection ID
    ///
    /// # Arguments
    ///
    /// * `connection` - A boxed connection that will be passed to the connection group.
    /// * `id` - a unique connection ID that will be used to send messages over a connection.
    ///
    /// If the add failed, a `ConnectionMatrixAddError` will be returned.
    fn add(
        &self,
        connection: Box<dyn Connection>,
        id: String,
    ) -> Result<usize, ConnectionMatrixAddError>;

    /// Removes a connection from the connection group with a connection ID.
    ///
    /// # Arguments
    ///
    /// * `id` - the unique connection ID for the connection that should be removed
    ///
    /// If the remove failed, a `ConnectionMatrixRemoveError` will be returned.
    fn remove(&self, id: &str) -> Result<Box<dyn Connection>, ConnectionMatrixRemoveError>;
}

/// `ConnectionMatrixSender` trait abstracts out sending messages through a connection group
/// without requiring knowledge about adding and removing connections.
pub trait ConnectionMatrixSender: Clone + Send {
    /// Sends a message over the specified connection.
    ///
    /// # Arguments
    ///
    /// * `id` - the unique connection ID the message should be sent over
    /// * `message` - the bytes of the message
    ///
    /// If the send failed, a `ConnectionMatrixSendError` will be returned.
    fn send(&self, id: String, message: Vec<u8>) -> Result<(), ConnectionMatrixSendError>;
}

/// `ConnectionMatrixReceiver` trait abstracts out receiving messages from a connection group without
/// requiring knowledge about adding and removing connections.
pub trait ConnectionMatrixReceiver: Clone + Send {
    /// Attempts to receive a message from the connection group. This function will block until
    /// there is a `ConnectionMatrixEnvelope` to receive. The message will come from the first ready connection
    /// detected.
    ///
    /// If successful, returns an `ConnectionMatrixEnvelope` containing the payload and the connection ID of the
    /// the sender. Otherwise, returns a `ConnectionMatrixRecvError`.
    fn recv(&self) -> Result<ConnectionMatrixEnvelope, ConnectionMatrixRecvError>;

    /// Attempts to receive a message from the connection group. This function will block until
    /// there is a `ConnectionMatrixEnvelope` to receive or the timeout expires
    ///
    /// # Arguments
    ///
    /// * `timeout` - a Duration for the amount of time the function should block waiting on an
    ///     envelope to arrive.
    ///
    /// If successful, returns an `ConnectionMatrixEnvelope` containing the payload and the connection ID of the
    /// the sender. Otherwise, returns a `ConnectionMatrixRecvTimeoutError`.
    fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> Result<ConnectionMatrixEnvelope, ConnectionMatrixRecvTimeoutError>;
}

/// `ConnectionMatrixShutdown` trait abstracts out shutting down a connection group without
/// requiring knowledge of the other connection group operations.
pub trait ConnectionMatrixShutdown: Clone + Send {
    /// Notifies the underlying connection group to shutdown
    fn shutdown(&self);
}
