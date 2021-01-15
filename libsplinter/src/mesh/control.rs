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

use mio_extras::channel as mio_channel;

use std::error::Error;
use std::fmt;
use std::io;

use crate::mesh::Outgoing;
use crate::transport::Connection;

/// Handle for adding and removing connections from backend
#[derive(Clone)]
pub struct Control {
    tx: mio_channel::Sender<ControlRequest>,
}

impl Control {
    pub(super) fn new(tx: mio_channel::Sender<ControlRequest>) -> Self {
        Control { tx }
    }

    pub fn add(&self, connection: Box<dyn Connection>) -> Result<Outgoing, AddError> {
        let (response_tx, response_rx) = crossbeam_channel::bounded(1);
        self.tx.send(ControlRequest::Add(AddRequest {
            connection,
            response_tx,
        }))?;
        match response_rx.recv() {
            Ok(Ok(outgoing)) => Ok(outgoing),
            Ok(Err(err)) => Err(err),
            Err(_err) => Err(AddError::ReceiverDisconnected),
        }
    }

    pub fn remove(&self, id: usize) -> Result<Box<dyn Connection>, RemoveError> {
        let (response_tx, response_rx) = crossbeam_channel::bounded(1);
        self.tx
            .send(ControlRequest::Remove(RemoveRequest { id, response_tx }))?;
        match response_rx.recv() {
            Ok(Ok(connection)) => Ok(connection),
            Ok(Err(err)) => Err(err),
            Err(_err) => Err(RemoveError::ReceiverDisconnected),
        }
    }

    pub fn shutdown(&self) {
        if self.tx.send(ControlRequest::Shutdown).is_err() {
            error!("Mesh has already shutdown")
        }
    }
}

pub(super) enum ControlRequest {
    Add(AddRequest),
    Remove(RemoveRequest),
    Shutdown,
}

pub(super) struct AddRequest {
    pub connection: Box<dyn Connection>,
    pub response_tx: crossbeam_channel::Sender<AddResponse>,
}

pub(super) type AddResponse = Result<Outgoing, AddError>;

pub enum AddError {
    Io(io::Error),
    SenderDisconnected(Box<dyn Connection>),
    ReceiverDisconnected,
    PoisonedLock,
}

impl Error for AddError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AddError::Io(err) => Some(err),
            AddError::SenderDisconnected(_) => None,
            AddError::ReceiverDisconnected => None,
            AddError::PoisonedLock => None,
        }
    }
}

impl fmt::Display for AddError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AddError::Io(ref err) => write!(f, "io error while trying to add connection {}", err),
            AddError::SenderDisconnected(_) => {
                write!(f, "unable to add connection, sender disconnected")
            }
            AddError::ReceiverDisconnected => {
                write!(f, "unable to add connection, receiver disconnected")
            }
            AddError::PoisonedLock => write!(f, "MeshState lock was poisoned"),
        }
    }
}

impl fmt::Debug for AddError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AddError::Io(ref err) => fmt::Debug::fmt(err, f),
            AddError::SenderDisconnected(_) => {
                write!(f, "AddError::SenderDisconnected(Box<dyn Connection>)")
            }
            AddError::ReceiverDisconnected => write!(f, "AddError::ReceiverDisconnected"),
            AddError::PoisonedLock => write!(f, "AddError::PoisonedLock"),
        }
    }
}

impl From<mio_channel::SendError<ControlRequest>> for AddError {
    fn from(err: mio_channel::SendError<ControlRequest>) -> Self {
        match err {
            mio_channel::SendError::Io(err) => AddError::Io(err),
            mio_channel::SendError::Disconnected(ControlRequest::Add(req)) => {
                AddError::SenderDisconnected(req.connection)
            }
            mio_channel::SendError::Disconnected(_req) => {
                panic!("Tried to convert ControlRequest that wasn't AddRequest to AddError")
            }
        }
    }
}

pub(super) struct RemoveRequest {
    pub id: usize,
    pub response_tx: crossbeam_channel::Sender<RemoveResponse>,
}

pub(super) type RemoveResponse = Result<Box<dyn Connection>, RemoveError>;

#[derive(Debug)]
pub enum RemoveError {
    Io(io::Error),
    NotFound,
    SenderDisconnected(usize),
    ReceiverDisconnected,
    PoisonedLock,
}

impl Error for RemoveError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RemoveError::Io(err) => Some(err),
            RemoveError::NotFound => None,
            RemoveError::SenderDisconnected(_) => None,
            RemoveError::ReceiverDisconnected => None,
            RemoveError::PoisonedLock => None,
        }
    }
}

impl fmt::Display for RemoveError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RemoveError::Io(ref err) => {
                write!(f, "io error while trying to remove connection {}", err)
            }
            RemoveError::NotFound => write!(f, "unable to remove connection, connection not found"),
            RemoveError::SenderDisconnected(_) => {
                write!(f, "unable to remove connection, sender disconnected")
            }
            RemoveError::ReceiverDisconnected => {
                write!(f, "unable to remove connection, receiver disconnected")
            }
            RemoveError::PoisonedLock => write!(f, "MeshState lock was poisoned"),
        }
    }
}

impl From<mio_channel::SendError<ControlRequest>> for RemoveError {
    fn from(err: mio_channel::SendError<ControlRequest>) -> Self {
        match err {
            mio_channel::SendError::Io(err) => RemoveError::Io(err),
            mio_channel::SendError::Disconnected(ControlRequest::Remove(req)) => {
                RemoveError::SenderDisconnected(req.id)
            }
            mio_channel::SendError::Disconnected(_req) => {
                panic!("Tried to convert ControlRequest that wasn't RemoveRequest to RemoveError")
            }
        }
    }
}
