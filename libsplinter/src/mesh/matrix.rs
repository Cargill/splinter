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
use std::time::Duration;

use crate::mesh::MeshShutdownSignaler;
use crate::transport::matrix::{
    ConnectionMatrixAddError, ConnectionMatrixEnvelope, ConnectionMatrixLifeCycle,
    ConnectionMatrixReceiver, ConnectionMatrixRecvError, ConnectionMatrixRecvTimeoutError,
    ConnectionMatrixRemoveError, ConnectionMatrixSendError, ConnectionMatrixSender,
    ConnectionMatrixShutdown,
};
use crate::transport::Connection;

use super::{Mesh, RecvError, RecvTimeoutError};

#[derive(Clone)]
/// Mesh specific implementation of ConnectionMatrixLifeCycle
pub struct MeshLifeCycle {
    mesh: Mesh,
}

impl MeshLifeCycle {
    pub fn new(mesh: Mesh) -> Self {
        MeshLifeCycle { mesh }
    }
}

impl ConnectionMatrixLifeCycle for MeshLifeCycle {
    fn add(
        &self,
        connection: Box<dyn Connection>,
        id: String,
    ) -> Result<usize, ConnectionMatrixAddError> {
        self.mesh.add(connection, id).map_err(|err| {
            ConnectionMatrixAddError::new(
                "Unable to add connection to matrix".to_string(),
                Some(Box::new(err)),
            )
        })
    }

    fn remove(&self, id: &str) -> Result<Box<dyn Connection>, ConnectionMatrixRemoveError> {
        self.mesh.remove(id).map_err(|err| {
            ConnectionMatrixRemoveError::new(
                "Unable to remove connection from matrix".to_string(),
                Some(Box::new(err)),
            )
        })
    }
}

#[derive(Clone)]
/// Mesh specific implementation of ConnectionMatrixSender
pub struct MeshMatrixSender {
    mesh: Mesh,
}

impl MeshMatrixSender {
    pub fn new(mesh: Mesh) -> Self {
        MeshMatrixSender { mesh }
    }
}

impl ConnectionMatrixSender for MeshMatrixSender {
    fn send(&self, id: String, message: Vec<u8>) -> Result<(), ConnectionMatrixSendError> {
        let envelope = ConnectionMatrixEnvelope::new(id, message);
        self.mesh.send(envelope).map_err(|err| {
            ConnectionMatrixSendError::new(
                "Unable to send message to connection".to_string(),
                Some(Box::new(err)),
            )
        })
    }
}

#[derive(Clone)]
/// Mesh specific implementation of MatrixReceiver
pub struct MeshMatrixReceiver {
    mesh: Mesh,
}

impl MeshMatrixReceiver {
    pub fn new(mesh: Mesh) -> Self {
        MeshMatrixReceiver { mesh }
    }
}

impl ConnectionMatrixReceiver for MeshMatrixReceiver {
    fn recv(&self) -> Result<ConnectionMatrixEnvelope, ConnectionMatrixRecvError> {
        match self.mesh.recv() {
            Ok(envelope) => Ok(envelope),
            Err(err) => match err {
                RecvError::Disconnected => Err(ConnectionMatrixRecvError::Disconnected),
                RecvError::PoisonedLock => Err(ConnectionMatrixRecvError::new_internal_error(
                    "Internal state poisoned".to_string(),
                    Some(Box::new(err)),
                )),
                RecvError::Shutdown => Err(ConnectionMatrixRecvError::Shutdown),
            },
        }
    }

    fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> Result<ConnectionMatrixEnvelope, ConnectionMatrixRecvTimeoutError> {
        match self.mesh.recv_timeout(timeout) {
            Ok(envelope) => Ok(envelope),
            Err(err) => match err {
                RecvTimeoutError::Timeout => Err(ConnectionMatrixRecvTimeoutError::Timeout),
                RecvTimeoutError::Disconnected => {
                    Err(ConnectionMatrixRecvTimeoutError::Disconnected)
                }
                RecvTimeoutError::PoisonedLock => {
                    Err(ConnectionMatrixRecvTimeoutError::new_internal_error(
                        "Internal state poisoned".to_string(),
                        Some(Box::new(err)),
                    ))
                }
                RecvTimeoutError::Shutdown => Err(ConnectionMatrixRecvTimeoutError::Shutdown),
            },
        }
    }
}

#[derive(Clone)]
/// Mesh specific implementation of MatrixShutdown
pub struct MeshMatrixShutdown {
    shutdown_signaler: MeshShutdownSignaler,
}

impl MeshMatrixShutdown {
    pub fn new(shutdown_signaler: MeshShutdownSignaler) -> Self {
        MeshMatrixShutdown { shutdown_signaler }
    }
}

impl ConnectionMatrixShutdown for MeshMatrixShutdown {
    fn shutdown(&self) {
        self.shutdown_signaler.shutdown();
    }
}
