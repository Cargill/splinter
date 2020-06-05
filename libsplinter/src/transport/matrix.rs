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

//! Traits for defining a connection matrix.
//!
//! A connection matrix is a collection containing [`Connection`] items, which allows sending to
//! and receiving from those connections via a connection identifier.
//!
//! A connection matrix must implement the following operations:
//!
//!   - add: Add a connection to the connection matrix
//!   - remove: Remove a connection from the connection matrix
//!   - send: Send a message to a connection
//!   - recv: Receive a message from any ready connection in the connection matrix
//!
//! Traits are defined in a granular manner which matches how a connection matrix is used. For
//! example, lifecycle operations (add and remove) are performed in a different component than send
//! and receive, and are thus handled via a separate trait.
//!
//! [`Connection`]: ../trait.Connection.html

use std::time::Duration;

use super::Connection;

pub use super::error::{
    ConnectionMatrixAddError, ConnectionMatrixRecvError, ConnectionMatrixRecvTimeoutError,
    ConnectionMatrixRemoveError, ConnectionMatrixSendError,
};

/// Contains a payload and the identifier for the connection on which the payload was received
#[derive(Debug, Default, PartialEq)]
pub struct ConnectionMatrixEnvelope {
    /// The connection identifier
    id: String,
    /// The message payload bytes
    payload: Vec<u8>,
}

impl ConnectionMatrixEnvelope {
    /// Creates a new `ConnectionMatrixEnvelope`
    ///
    /// This is used by the implementation of a [`ConnectionMatrixReceiver`] to create the envelope
    /// returned by [`recv`] or [`recv_timeout`].
    ///
    /// [`ConnectionMatrixReceiver`]: trait.ConnectionMatrixReceiver.html
    /// [`recv`]: trait.ConnectionMatrixReceiver.html#tymethod.recv
    /// [`recv_timeout`]: trait.ConnectionMatrixReceiver.html#tymethod.recv_timeout
    pub fn new(id: String, payload: Vec<u8>) -> Self {
        ConnectionMatrixEnvelope { id, payload }
    }

    /// Returns the connection identifier of the connection on which the payload was received
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the bytes of the payload
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Returns the bytes of the payload while consuming the `ConnectionMatrixEnvelope`
    #[deprecated(since = "0.4.1", note = "Please use into_inner() instead")]
    pub fn take_payload(self) -> Vec<u8> {
        self.payload
    }

    /// Returns the payload and consumes the ConnectionMatrixEnvelope
    pub fn into_inner(self) -> Vec<u8> {
        self.payload
    }
}

impl From<ConnectionMatrixEnvelope> for Vec<u8> {
    fn from(envelope: ConnectionMatrixEnvelope) -> Self {
        envelope.payload.to_vec()
    }
}

/// Defines connection lifecycle operations (addition and removal of a `Connection`)
///
/// This trait is distinct from the sender/receiver traits because the lifecycle operations
/// typically occur in a separate component. Thus, we can expose this trait only where the
/// lifecycle operations are performed and nowhere else in the system.
pub trait ConnectionMatrixLifeCycle: Clone + Send {
    /// Adds a connection to the connection matrix
    ///
    /// # Arguments
    ///
    /// * `connection` - Connection being added to the connection matrix
    /// * `id` - Connection identifier; must be unique within the connection matrix
    ///
    /// If the add failed, a `ConnectionMatrixAddError` will be returned.
    fn add(
        &self,
        connection: Box<dyn Connection>,
        id: String,
    ) -> Result<usize, ConnectionMatrixAddError>;

    /// Removes a connection from the connection matrix
    ///
    /// # Arguments
    ///
    /// * `id` - the connection identifier for the connection being removed
    ///
    /// If the remove failed, a `ConnectionMatrixRemoveError` will be returned.
    fn remove(&self, id: &str) -> Result<Box<dyn Connection>, ConnectionMatrixRemoveError>;
}

/// Defines a function to send a message using a connection identifier
pub trait ConnectionMatrixSender: Clone + Send {
    /// Sends a message over the specified connection.
    ///
    /// # Arguments
    ///
    /// * `id` - the identifier of the connection on which the message should be sent
    /// * `message` - the bytes of the message
    ///
    /// If the send failed, a `ConnectionMatrixSendError` will be returned.
    fn send(&self, id: String, message: Vec<u8>) -> Result<(), ConnectionMatrixSendError>;
}

/// Defines functions to receive messages from connections within the connection matrix
pub trait ConnectionMatrixReceiver: Clone + Send {
    /// Attempts to receive a message. The envelope returned contains both the payload (message)
    /// and the identifier of the connection on which it was received. This function will block
    /// until there is a message to receive. The message will come from the first ready connection
    /// detected.
    ///
    /// If the receive failed, a `ConnectionMatrixRecvError` is returned.
    fn recv(&self) -> Result<ConnectionMatrixEnvelope, ConnectionMatrixRecvError>;

    /// Attempts to receive a message, with a timeout. The envelope returned contains both the
    /// payload (message) and the identifier of the connection on which it was received. This
    /// function will block until there is a message to receive or the specified timeout expires.
    /// The message will come from the first ready connection detected.
    ///
    /// # Arguments
    ///
    /// * `timeout` - `Duration` for the amount of time the function should block waiting on an
    ///   envelope to arrive
    ///
    /// If the receive failed or timed out, a `ConnectionMatrixRecvTimeoutError` is returned.
    fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> Result<ConnectionMatrixEnvelope, ConnectionMatrixRecvTimeoutError>;
}

/// Defines a function to shutdown the connection matrix
pub trait ConnectionMatrixShutdown: Clone + Send {
    /// Notifies the underlying connection matrix to shutdown
    fn shutdown(&self);
}
