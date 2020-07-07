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

//! Data structures for communicating with the PeerManager.

use std::sync::mpsc::{channel, Sender};

use crate::collections::BiHashMap;

use super::error::{
    PeerConnectionIdError, PeerListError, PeerLookupError, PeerManagerError, PeerRefAddError,
    PeerRefRemoveError, PeerUnknownAddError,
};
use super::notification::{PeerManagerNotification, PeerNotificationIter, SubscriberId};
use super::{EndpointPeerRef, PeerRef};
use super::{PeerManagerMessage, PeerManagerRequest};

/// The `PeerLookup` trait provides an interface for looking up details about individual peer
/// connections.
pub trait PeerLookup: Send {
    /// Retrieves the connection ID for a given peer ID, if found.
    ///
    /// # Errors
    ///
    /// Returns a `PeerLookupError` if the connection ID cannot be retrieved.
    fn connection_id(&self, peer_id: &str) -> Result<Option<String>, PeerLookupError>;

    /// Retrieves the peer ID for a given connection ID, if found.
    ///
    /// # Errors
    ///
    /// Returns a `PeerLookupError` if the peer ID cannot be retrieved.
    fn peer_id(&self, connection_id: &str) -> Result<Option<String>, PeerLookupError>;
}

pub trait PeerLookupProvider {
    fn peer_lookup(&self) -> Box<dyn PeerLookup>;
}

/// The `PeerManagerConnector` will be used to make requests to the `PeerManager`.
///
/// The connector includes functions to add a new peer reference, update a peer and list the
/// existing peers.
#[derive(Clone, Debug)]
pub struct PeerManagerConnector {
    sender: Sender<PeerManagerMessage>,
}

impl PeerManagerConnector {
    pub(crate) fn new(sender: Sender<PeerManagerMessage>) -> Self {
        PeerManagerConnector { sender }
    }

    /// Requests that a peer is added to the `PeerManager`. If a peer already exists, the peer's
    /// reference count will be incremented
    ///
    /// Returns a `PeerRef` that, when dropped, will automatically send a removal request to the
    /// `PeerManager`.
    ///
    /// # Arguments
    ///
    /// * `peer_id` -  The unique ID for the peer.
    /// * `endpoints` -  The list of endpoints associated with the peer. The list should be in
    ///   order of preference, with the first endpoint being the first attempted.
    pub fn add_peer_ref(
        &self,
        peer_id: String,
        endpoints: Vec<String>,
    ) -> Result<PeerRef, PeerRefAddError> {
        let (sender, recv) = channel();

        let message = PeerManagerMessage::Request(PeerManagerRequest::AddPeer {
            peer_id,
            endpoints,
            sender,
        });

        match self.sender.send(message) {
            Ok(()) => (),
            Err(_) => {
                return Err(PeerRefAddError::InternalError(
                    "Unable to send message to PeerManager, receiver dropped".to_string(),
                ))
            }
        };

        recv.recv()
            .map_err(|err| PeerRefAddError::ReceiveError(format!("{:?}", err)))?
    }

    /// Requests that a peer is added to the `PeerManager`. This function should be used when the
    /// peer ID is unknown.
    ///
    /// Returns `Ok(EndpointPeerRef)` if the unidentified peer was added
    ///
    /// # Arguments
    ///
    /// * `endpoint` - The endpoint associated with the peer.
    pub fn add_unidentified_peer(
        &self,
        endpoint: String,
    ) -> Result<EndpointPeerRef, PeerUnknownAddError> {
        let (sender, recv) = channel();

        let message =
            PeerManagerMessage::Request(PeerManagerRequest::AddUnidentified { endpoint, sender });

        match self.sender.send(message) {
            Ok(()) => (),
            Err(_) => {
                return Err(PeerUnknownAddError::InternalError(
                    "Unable to send message to PeerManager, receiver dropped".to_string(),
                ))
            }
        };

        recv.recv()
            .map_err(|err| PeerUnknownAddError::ReceiveError(format!("{:?}", err)))?
    }

    /// Requests the list of currently connected peers.
    ///
    /// Returns the list of peer IDs.
    pub fn list_peers(&self) -> Result<Vec<String>, PeerListError> {
        let (sender, recv) = channel();
        let message = PeerManagerMessage::Request(PeerManagerRequest::ListPeers { sender });

        match self.sender.send(message) {
            Ok(()) => (),
            Err(_) => {
                return Err(PeerListError::InternalError(
                    "Unable to send message to PeerManager, receiver dropped".to_string(),
                ))
            }
        };

        recv.recv()
            .map_err(|err| PeerListError::ReceiveError(format!("{:?}", err)))?
    }

    /// Requests the list of unreferenced peers.
    ///
    /// Unreferenced peers are those peers that have successfully connected from a remote node, but
    /// have not yet been referenced by a circuit. These peers are available to be promoted to
    /// fully refrerenced peers.
    pub fn list_unreferenced_peers(&self) -> Result<Vec<String>, PeerListError> {
        let (sender, recv) = channel();
        let message =
            PeerManagerMessage::Request(PeerManagerRequest::ListUnreferencedPeers { sender });

        match self.sender.send(message) {
            Ok(()) => (),
            Err(_) => {
                return Err(PeerListError::InternalError(
                    "Unable to send message to PeerManager, receiver dropped".to_string(),
                ))
            }
        };

        recv.recv()
            .map_err(|err| PeerListError::ReceiveError(format!("{:?}", err)))?
    }

    /// Requests the map of currently connected peers to connection IDs
    ///
    /// Returns a map of peer IDs to connection IDs
    pub fn connection_ids(&self) -> Result<BiHashMap<String, String>, PeerConnectionIdError> {
        let (sender, recv) = channel();
        let message = PeerManagerMessage::Request(PeerManagerRequest::ConnectionIds { sender });

        match self.sender.send(message) {
            Ok(()) => (),
            Err(_) => {
                return Err(PeerConnectionIdError::InternalError(
                    "Unable to send message to PeerManager, receiver dropped".to_string(),
                ))
            }
        };

        recv.recv()
            .map_err(|err| PeerConnectionIdError::ReceiveError(format!("{:?}", err)))?
    }

    /// Subscribes to `PeerManager` notifications.
    ///
    /// Returns a `PeerNotificationIter` that can be used to receive notifications about connected
    /// and disconnected peers
    #[deprecated(since = "0.5.1", note = "please use `subscribe_sender` instead")]
    pub fn subscribe(&self) -> Result<PeerNotificationIter, PeerManagerError> {
        let (send, recv) = channel();
        match self.sender.send(PeerManagerMessage::Subscribe(send)) {
            Ok(()) => Ok(PeerNotificationIter { recv }),
            Err(_) => Err(PeerManagerError::SendMessageError(
                "The peer manager is no longer running".into(),
            )),
        }
    }

    /// Subscribe to notifications for peer events.
    ///
    /// `PeerManagerNotification` instances will be transformed via type `T`'s implementation
    /// of `From<PeerManagerNotification>` and passed to the given sender.
    ///
    /// # Returns
    ///
    /// The subscriber ID that can be used for unsubscribing the given sender.
    ///
    /// # Errors
    ///
    /// Return a `PeerManagerError` if the subscriber cannot be registered via the
    /// `PeerManagerConnector` instance.
    pub fn subscribe_sender<T>(
        &self,
        subscriber: Sender<T>,
    ) -> Result<SubscriberId, PeerManagerError>
    where
        T: From<PeerManagerNotification> + Send + 'static,
    {
        let (sender, recv) = channel();
        self.sender
            .send(PeerManagerMessage::Request(PeerManagerRequest::Subscribe {
                sender,
                callback: Box::new(move |notification| {
                    subscriber.send(T::from(notification)).map_err(Box::from)
                }),
            }))
            .map_err(|_| {
                PeerManagerError::SendMessageError("The peer manager is no longer running".into())
            })?;

        recv.recv().map_err(|_| {
            PeerManagerError::SendMessageError("The peer manager is no longer running".into())
        })?
    }

    /// Unsubscribe from `PeerManagerNotification`.
    ///
    /// # Errors
    ///
    /// Returns a `PeerManagerError` if the `PeerManager` has stopped running.
    pub fn unsubscribe(&self, subscriber_id: SubscriberId) -> Result<(), PeerManagerError> {
        let (sender, recv) = channel();
        self.sender
            .send(PeerManagerMessage::Request(
                PeerManagerRequest::Unsubscribe {
                    subscriber_id,
                    sender,
                },
            ))
            .map_err(|_| {
                PeerManagerError::SendMessageError("The peer manager is no longer running".into())
            })?;

        recv.recv().map_err(|_| {
            PeerManagerError::SendMessageError("The peer manager is no longer running".into())
        })?
    }
}

impl PeerLookup for PeerManagerConnector {
    fn connection_id(&self, peer_id: &str) -> Result<Option<String>, PeerLookupError> {
        let (sender, recv) = channel();
        let message = PeerManagerMessage::Request(PeerManagerRequest::GetConnectionId {
            peer_id: peer_id.to_string(),
            sender,
        });

        match self.sender.send(message) {
            Ok(()) => (),
            Err(_) => {
                return Err(PeerLookupError(
                    "Unable to send message to PeerManager, receiver dropped".to_string(),
                ))
            }
        };

        recv.recv()
            .map_err(|err| PeerLookupError(format!("{:?}", err)))?
    }

    fn peer_id(&self, connection_id: &str) -> Result<Option<String>, PeerLookupError> {
        let (sender, recv) = channel();
        let message = PeerManagerMessage::Request(PeerManagerRequest::GetPeerId {
            connection_id: connection_id.to_string(),
            sender,
        });

        match self.sender.send(message) {
            Ok(()) => (),
            Err(_) => {
                return Err(PeerLookupError(
                    "Unable to send message to PeerManager, receiver dropped".to_string(),
                ))
            }
        };

        recv.recv()
            .map_err(|err| PeerLookupError(format!("{:?}", err)))?
    }
}

impl PeerLookupProvider for PeerManagerConnector {
    fn peer_lookup(&self) -> Box<dyn PeerLookup> {
        Box::new(self.clone())
    }
}

/// The `PeerRemover` will be used by `PeerRef` to decrement the reference count for a peer when
/// the `PeerRef` is dropped.
#[derive(Clone, Debug)]
pub(crate) struct PeerRemover {
    pub sender: Sender<PeerManagerMessage>,
}

impl PeerRemover {
    /// Sends a request to the `PeerManager` to remove a peer.
    ///
    /// This function will only be called when the PeerRef is dropped.
    ///
    /// # Arguments
    /// * `peer_id` - the peer ID of the `PeerRef` that has been dropped
    pub fn remove_peer_ref(&self, peer_id: &str) -> Result<(), PeerRefRemoveError> {
        let (sender, recv) = channel();

        let message = PeerManagerMessage::Request(PeerManagerRequest::RemovePeer {
            peer_id: peer_id.to_string(),
            sender,
        });

        match self.sender.send(message) {
            Ok(()) => (),
            Err(_) => {
                return Err(PeerRefRemoveError::InternalError(
                    "Unable to send message to PeerManager, receiver dropped".to_string(),
                ))
            }
        };

        recv.recv()
            .map_err(|err| PeerRefRemoveError::ReceiveError(format!("{:?}", err)))?
    }

    /// Sends a request to the `PeerManager` to remove a peer by its endpoint.
    ///
    /// This function will only be called when the `EndpointPeerRef` is dropped.
    ///
    /// # Arguments
    /// * `endpoint` - the endpoint of the `EndpointPeerRef` that has been dropped
    pub fn remove_peer_ref_by_endpoint(&self, endpoint: &str) -> Result<(), PeerRefRemoveError> {
        let (sender, recv) = channel();

        let message = PeerManagerMessage::Request(PeerManagerRequest::RemovePeerByEndpoint {
            endpoint: endpoint.to_string(),
            sender,
        });

        match self.sender.send(message) {
            Ok(()) => (),
            Err(_) => {
                return Err(PeerRefRemoveError::InternalError(
                    "Unable to send message to PeerManager, receiver dropped".to_string(),
                ))
            }
        };

        recv.recv()
            .map_err(|err| PeerRefRemoveError::ReceiveError(format!("{:?}", err)))?
    }
}

impl PartialEq for PeerRemover {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}
