// Copyright 2018-2021 Cargill Incorporated
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

//! Mesh is an asynchronous Connection handler that sends and receives across many Connections in a
//! background thread.
//!
//!     use splinter::{mesh::{Envelope, Mesh}, transport::{Transport, inproc::InprocTransport}};
//!
//!     let mut transport = InprocTransport::default();
//!     let mesh = Mesh::new(1, 1);
//!     let mut listener = transport.listen("inproc://my-connection").unwrap();
//!
//!     mesh.add(transport.connect(&listener.endpoint()).unwrap(), "client".to_string()).unwrap();
//!     mesh.add(listener.accept().unwrap(), "server".to_string()).unwrap();
//!
//!     mesh.send(Envelope::new("client".to_string(), b"hello".to_vec())).unwrap();
//!     mesh.recv().unwrap();
//!
//!     let client = mesh.remove("client").unwrap();
//!     // If we were to drop client above, the reactor could detect that client disconnected from
//!     // the server and automatically cleanup and remove server, causing this to fail with
//!     // RemoveError::NotFound.
//!     let server = mesh.remove("server").unwrap();
//!
//! Mesh can be cloned relatively cheaply and passed between threads. If receiving is performed
//! from many clones, envelopes will be distributed among them.
//!
//! The following goals influenced this implementation:
//!
//! 1. The main reactor in the background thread should hold no locks. Adding a single RwLock read
//!    acquisition to an otherwise simple event loop was observed to decrease performance by a
//!    factor of 5x.
//! 2. Sends to connections should be queued and handled independently. This means if one
//!    Connection has a bunch of sends queued but its underlying socket is not writable, other
//!    Connections must still be able to send. This implementation uses a separate outgoing queue
//!    for each Connection that can be polled in the event loop to accomplish this, but there may
//!    be a more efficient implementation.
//! 3. Backpressure should be built in. This means all queues should be bounded so that a
//!    backpressure error can be returned when the queue is full.

mod control;
mod incoming;
mod matrix;
mod outgoing;
mod pool;
mod reactor;

use std::collections::HashMap;
use std::error::Error;
use std::io;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use crate::mesh::control::Control;
pub use crate::mesh::control::{AddError, RemoveError};
use crate::mesh::incoming::Incoming;
pub use crate::mesh::matrix::{
    MeshLifeCycle, MeshMatrixReceiver, MeshMatrixSender, MeshMatrixShutdown,
};
use crate::mesh::outgoing::Outgoing;
pub use crate::transport::matrix::ConnectionMatrixEnvelope as Envelope;

use crate::collections::BiHashMap;
use crate::mesh::reactor::Reactor;
use crate::transport::Connection;

/// Wrapper around payload to include connection id
#[derive(Debug, PartialEq)]
pub(in crate::mesh) enum InternalEnvelope {
    Message { id: usize, payload: Vec<u8> },
    Shutdown,
}

struct MeshState {
    pub outgoings: HashMap<usize, Outgoing>,
    pub unique_ids: BiHashMap<String, usize>,
}

impl MeshState {
    fn new() -> Self {
        MeshState {
            outgoings: HashMap::new(),
            unique_ids: BiHashMap::new(),
        }
    }
}

#[derive(Clone)]
pub struct MeshShutdownSignaler {
    ctrl: Control,
}

impl MeshShutdownSignaler {
    pub fn shutdown(&self) {
        self.ctrl.shutdown();
    }
}

/// A Connection reactor
#[derive(Clone)]
pub struct Mesh {
    state: Arc<RwLock<MeshState>>,
    incoming: Incoming,
    ctrl: Control,
}

impl Mesh {
    /// Create a new mesh, spawning a background thread for sending and receiving, and setting up
    /// channels to communicate with it.
    pub fn new(incoming_capacity: usize, outgoing_capacity: usize) -> Self {
        let (ctrl, incoming) = Reactor::spawn(incoming_capacity, outgoing_capacity);
        Mesh {
            state: Arc::new(RwLock::new(MeshState::new())),
            incoming,
            ctrl,
        }
    }

    pub fn shutdown_signaler(&self) -> MeshShutdownSignaler {
        MeshShutdownSignaler {
            ctrl: self.ctrl.clone(),
        }
    }

    /// Add a new connection to the mesh, moving it to the background thread, and add the returned
    /// mesh id to the unique_ids map
    pub fn add(
        &self,
        connection: Box<dyn Connection>,
        unique_id: String,
    ) -> Result<usize, AddError> {
        let mut state = self.state.write().map_err(|_| AddError::PoisonedLock)?;
        let outgoing = self.ctrl.add(connection)?;
        let mesh_id = outgoing.id();

        state.outgoings.insert(mesh_id, outgoing);
        state.unique_ids.insert(unique_id, mesh_id);

        Ok(mesh_id)
    }

    /// Remove an existing connection from the mesh and return it.
    pub fn remove(&self, unique_id: &str) -> Result<Box<dyn Connection>, RemoveError> {
        let mut state = self.state.write().map_err(|_| RemoveError::PoisonedLock)?;
        if let Some((_, mesh_id)) = state.unique_ids.remove_by_key(unique_id) {
            let connection = self.ctrl.remove(mesh_id)?;
            // The outgoing channel needs to be removed after the control request completes, or else
            // the reactor will detect that the outgoing sender has dropped and clean it up
            // automatically, causing the control request to fail with NotFound.

            state.outgoings.remove(&mesh_id);
            Ok(connection)
        } else {
            Err(RemoveError::NotFound)
        }
    }

    /// Send the envelope on the mesh.
    ///
    /// This is a convenience function and is equivalent to
    /// `mesh.outgoing(envelope.id()).send(Vec::from(envelope))`.
    pub fn send(&self, envelope: Envelope) -> Result<(), SendError> {
        let state = &self.state.read().map_err(|_| SendError::PoisonedLock)?;
        let id = envelope.id().to_string();
        if let Some(mesh_id) = state.unique_ids.get_by_key(&id) {
            match state.outgoings.get(mesh_id) {
                Some(ref outgoing) => match outgoing.send(Vec::from(envelope)) {
                    Ok(()) => Ok(()),
                    Err(err) => Err(SendError::from_outgoing_send_error(err, id)),
                },
                None => Err(SendError::NotFound),
            }
        } else {
            Err(SendError::NotFound)
        }
    }

    /// Receive a new envelope from the mesh.
    pub fn recv(&self) -> Result<Envelope, RecvError> {
        let internal_envelope = self.incoming.recv().map_err(|_| RecvError::Disconnected)?;
        match internal_envelope {
            InternalEnvelope::Shutdown => Err(RecvError::Shutdown),
            InternalEnvelope::Message {
                id: connection_id,
                payload,
            } => {
                let id = self
                    .state
                    .read()
                    .map_err(|_| RecvError::PoisonedLock)?
                    .unique_ids
                    .get_by_value(&connection_id)
                    .cloned()
                    .unwrap_or_default();

                Ok(Envelope::new(id, payload))
            }
        }
    }

    /// Receive a new envelope from the mesh.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<Envelope, RecvTimeoutError> {
        let internal_envelope = self
            .incoming
            .recv_timeout(timeout)
            .map_err(RecvTimeoutError::from)?;
        match internal_envelope {
            InternalEnvelope::Shutdown => Err(RecvTimeoutError::Shutdown),
            InternalEnvelope::Message {
                id: connection_id,
                payload,
            } => {
                let id = self
                    .state
                    .read()
                    .map_err(|_| RecvTimeoutError::PoisonedLock)?
                    .unique_ids
                    .get_by_value(&connection_id)
                    .cloned()
                    .unwrap_or_default();

                Ok(Envelope::new(id, payload))
            }
        }
    }

    /// Creates a MeshLifeCycle that can be used to add and remove connection from this Mesh
    pub fn get_life_cycle(&self) -> MeshLifeCycle {
        let mesh = self.clone();
        MeshLifeCycle::new(mesh)
    }

    /// Creates a MeshMatrixSender that can be used to send messages over through this Mesh
    pub fn get_sender(&self) -> MeshMatrixSender {
        let mesh = self.clone();
        MeshMatrixSender::new(mesh)
    }

    /// Creates a MeshMatrixReceiver that can be used to receives message from this Mesh
    pub fn get_receiver(&self) -> MeshMatrixReceiver {
        let mesh = self.clone();
        MeshMatrixReceiver::new(mesh)
    }

    /// Creates a MeshMatrixShutdown to shutdown this Mesh instance
    pub fn get_matrix_shutdown(&self) -> MeshMatrixShutdown {
        MeshMatrixShutdown::new(self.shutdown_signaler())
    }
}

#[derive(Debug)]
pub enum SendError {
    NotFound,
    IoError(io::Error),
    Full(Envelope),
    Disconnected(Envelope),
    PoisonedLock,
}

impl Error for SendError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SendError::NotFound => None,
            SendError::IoError(err) => Some(err),
            SendError::Full(_) => None,
            SendError::Disconnected(_) => None,
            SendError::PoisonedLock => None,
        }
    }
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            SendError::NotFound => write!(f, "requested connection cannot be found"),
            SendError::IoError(ref err) => write!(f, "io error processing message {}", err),
            SendError::Full(ref envelope) => {
                write!(f, "connection {} send queue is full", envelope.id())
            }
            SendError::Disconnected(ref envelope) => {
                write!(f, "connection disconnected {}", envelope.id())
            }
            SendError::PoisonedLock => write!(f, "MeshState lock was poisoned"),
        }
    }
}

impl SendError {
    fn from_outgoing_send_error(err: outgoing::SendError, id: String) -> Self {
        match err {
            outgoing::SendError::IoError(err) => SendError::IoError(err),
            outgoing::SendError::Full(payload) => SendError::Full(Envelope::new(id, payload)),
            outgoing::SendError::Disconnected(payload) => {
                SendError::Disconnected(Envelope::new(id, payload))
            }
        }
    }
}

#[derive(Debug)]
pub enum RecvError {
    Disconnected,
    PoisonedLock,
    Shutdown,
}

impl Error for RecvError {}

impl std::fmt::Display for RecvError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            RecvError::Disconnected => write!(f, "Unable to receive: channel has disconnected"),
            RecvError::PoisonedLock => write!(f, "MeshState lock was poisoned"),
            RecvError::Shutdown => write!(f, "Mesh has shutdown"),
        }
    }
}

#[derive(Debug)]
pub enum RecvTimeoutError {
    Timeout,
    Disconnected,
    PoisonedLock,
    Shutdown,
}

impl Error for RecvTimeoutError {}

impl std::fmt::Display for RecvTimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            RecvTimeoutError::Timeout => f.write_str("Unable to receive: Timeout"),
            RecvTimeoutError::Disconnected => {
                f.write_str("Unable to receive: channel has disconnected")
            }
            RecvTimeoutError::PoisonedLock => write!(f, "MeshState lock was poisoned"),
            RecvTimeoutError::Shutdown => write!(f, "Mesh has shutdown"),
        }
    }
}

impl From<incoming::RecvTimeoutError> for RecvTimeoutError {
    fn from(err: incoming::RecvTimeoutError) -> Self {
        match err {
            incoming::RecvTimeoutError::Timeout => RecvTimeoutError::Timeout,
            incoming::RecvTimeoutError::Disconnected => RecvTimeoutError::Disconnected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fmt::Debug;
    use std::sync::mpsc::channel;
    use std::thread;

    use crate::transport::{
        socket::tests::create_test_tls_transport, socket::TcpTransport, Transport,
    };

    fn assert_ok<T, E: Debug>(result: Result<T, E>) -> T {
        match result {
            Ok(ok) => ok,
            Err(err) => panic!("Expected Ok(...), got Err({:?})", err),
        }
    }

    // Test that connections added to a Mesh can send and receive successfully
    fn test_single_connection_send_receive<T: Transport + Send + 'static>(
        mut transport: T,
        bind: &str,
    ) {
        let mut listener = assert_ok(transport.listen(bind));
        let endpoint = listener.endpoint();

        let handle = thread::spawn(move || {
            let client = assert_ok(transport.connect(&endpoint));

            let mesh = Mesh::new(1, 1);
            assert_ok(mesh.add(client, "client".to_string()));

            assert_ok(mesh.send(Envelope::new("client".to_string(), b"hello".to_vec())));
        });

        let mesh = Mesh::new(1, 1);
        let server = assert_ok(listener.accept());
        assert_ok(mesh.add(server, "server".to_string()));

        let envelope = assert_ok(mesh.recv());

        assert_eq!(b"hello", envelope.payload());
        assert_eq!("server", envelope.id());

        handle.join().unwrap();
    }

    // Test that connections can be added and removed from a Mesh
    fn test_add_remove_connections<T: Transport + Send + 'static>(mut transport: T, bind: &str) {
        let mut listener = assert_ok(transport.listen(bind));
        let endpoint = listener.endpoint();

        let mesh = Mesh::new(0, 0);

        let mut ids = Vec::new();

        let mesh_clone = mesh.clone();
        let handle = thread::spawn(move || {
            let mesh = mesh_clone;

            let mut ids = Vec::new();
            for i in 0..8 {
                let conn = assert_ok(transport.connect(&endpoint));
                let id = format!("thread_{}", i);
                assert_ok(mesh.add(conn, id.clone()));
                ids.push(id);
            }

            ids
        });

        for i in 0..8 {
            let conn = assert_ok(listener.accept());
            let id = format!("main_{}", i);
            assert_ok(mesh.add(conn, id.clone()));
            ids.push(id);
        }

        ids.extend(handle.join().unwrap());
        ids.sort();

        for id in &ids {
            assert_ok(mesh.remove(id));
        }
    }

    // Test that many connections can be added to a Mesh and sent and received from
    fn test_many_connections<T: Transport + Send + 'static>(mut transport: T, bind: &str) {
        const CONNECTIONS: usize = 16;

        let mut listener = assert_ok(transport.listen(bind));
        let endpoint = listener.endpoint();

        let mesh = Mesh::new(CONNECTIONS, CONNECTIONS);

        let (client_ready_tx, client_ready_rx) = channel();
        let (server_ready_tx, server_ready_rx) = channel();

        let mesh_clone = mesh.clone();
        let handle = thread::spawn(move || {
            let mesh = mesh_clone;

            for i in 0..CONNECTIONS {
                let id = format!("thread_{}", i);
                assert_ok(mesh.add(assert_ok(transport.connect(&endpoint)), id));
            }

            // Block waiting for other thread to send everything
            client_ready_rx.recv().unwrap();

            let mut ids = Vec::with_capacity(CONNECTIONS);
            for _ in 0..CONNECTIONS {
                let envelope = assert_ok(mesh.recv());
                assert_eq!(b"hello", envelope.payload());
                ids.push(envelope.id().to_string());
            }

            // Signal to other thread we are done receiving
            server_ready_tx.send(()).unwrap();

            for id in ids {
                assert_ok(mesh.send(Envelope::new(id.to_string(), b"world".to_vec())));
            }
        });

        for i in 0..CONNECTIONS {
            let conn = assert_ok(listener.accept());
            let id = format!("main_{}", i);
            assert_ok(mesh.add(conn, id.clone()));
            assert_ok(mesh.send(Envelope::new(id, b"hello".to_vec())));
        }

        // Signal done sending to background thread
        client_ready_tx.send(()).unwrap();
        // Wait for other thread to drain the queue so we don't accidentally receive messages sent
        // to that thread
        server_ready_rx.recv().unwrap();

        let incoming = mesh.incoming.clone();
        for _ in 0..CONNECTIONS {
            let envelope = assert_ok(incoming.recv());
            match envelope {
                InternalEnvelope::Message { payload, .. } => assert_eq!(b"world".to_vec(), payload),
                InternalEnvelope::Shutdown => panic!("Should not have received shutdown"),
            }
        }

        handle.join().unwrap();
    }

    #[test]
    fn test_connection_send_receive_raw() {
        let raw = TcpTransport::default();
        test_single_connection_send_receive(raw, "127.0.0.1:0");
    }

    #[test]
    fn test_connection_send_receive_tls() {
        let tls = create_test_tls_transport(true);
        test_single_connection_send_receive(tls, "127.0.0.1:0");
    }

    #[test]
    fn test_add_remove_connections_raw() {
        let raw = TcpTransport::default();
        test_add_remove_connections(raw, "127.0.0.1:0");
    }

    #[test]
    fn test_many_connections_raw() {
        let raw = TcpTransport::default();
        test_many_connections(raw, "127.0.0.1:0");
    }

    #[test]
    fn test_many_connections_tls() {
        let tls = create_test_tls_transport(true);
        test_many_connections(tls, "127.0.0.1:0");
    }

    #[test]
    fn test_add_remove_connections_tls() {
        let tls = create_test_tls_transport(true);
        test_add_remove_connections(tls, "127.0.0.1:0");
    }

    #[test]
    // Test that mesh can be shutdown after sending and receiving a message.
    //
    // 1. Start up Mesh and pass a clone to a thread. Also get a mesh shutdown signaler.
    // 2. Add two connections to mesh, one for main and on for the thread.
    // 3. From the main thread, send a message "hello"
    // 4. From the other thread recv the message "hello", and respond with "world"
    // 5. In the main thread, verify that "world" is received
    // 6. Call shutdown on the shutdown signaler
    // 7. Verify that an InternalEnvelope::Shutdown was received
    //
    // This verifies that that mesh's reactor has been shutdown because InternalEnvelope::Shutdown
    // is only ever sent from within the reactor when it has received a shutdown request. Other
    // scenarios can cause the reactor to shutdown, but they will send this signal and result in a
    // disconnection error.
    fn test_shutdown() {
        let mut transport = TcpTransport::default();
        let mut listener = assert_ok(transport.listen("127.0.0.1:0"));
        let endpoint = listener.endpoint();

        let mesh = Mesh::new(1, 1);

        let (client_ready_tx, client_ready_rx) = channel();
        let (server_ready_tx, server_ready_rx) = channel();

        let mesh_clone = mesh.clone();
        let handle = thread::spawn(move || {
            let mesh = mesh_clone;
            assert_ok(mesh.add(
                assert_ok(transport.connect(&endpoint)),
                "thread_1".to_string(),
            ));

            // Block waiting for other thread to send a message
            client_ready_rx.recv().unwrap();

            let envelope = assert_ok(mesh.recv());
            assert_eq!(b"hello", envelope.payload());

            // Signal to other thread we are done receiving
            server_ready_tx.send(()).unwrap();

            assert_ok(mesh.send(Envelope::new("thread_1".to_string(), b"world".to_vec())));
        });

        let conn = assert_ok(listener.accept());
        assert_ok(mesh.add(conn, "main".to_string()));
        assert_ok(mesh.send(Envelope::new("main".to_string(), b"hello".to_vec())));

        // Signal done sending to background thread
        client_ready_tx.send(()).unwrap();
        // Wait for other thread to drain the queue so we don't accidentally receive messages sent
        // to that thread
        server_ready_rx.recv().unwrap();

        let incoming = mesh.incoming.clone();
        let envelope = assert_ok(incoming.recv());
        match envelope {
            InternalEnvelope::Message { payload, .. } => assert_eq!(b"world".to_vec(), payload),
            InternalEnvelope::Shutdown => panic!("Should not have received shutdown"),
        }

        let signaler = mesh.shutdown_signaler();
        signaler.shutdown();

        let envelope = assert_ok(incoming.recv());
        match envelope {
            InternalEnvelope::Message { .. } => panic!("Should have received shutdown"),
            InternalEnvelope::Shutdown => (),
        };

        handle.join().unwrap();
    }
}
