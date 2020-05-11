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

mod connector;
mod error;
pub mod interconnect;
mod notification;
mod peer_map;

use std::cmp::min;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::time::Instant;

use self::error::{
    PeerConnectionIdError, PeerListError, PeerLookupError, PeerManagerError, PeerRefAddError,
    PeerRefRemoveError, PeerUnknownAddError,
};
use crate::collections::{BiHashMap, RefMap};
use crate::network::connection_manager::ConnectionManagerNotification;
use crate::network::connection_manager::Connector;
pub use crate::network::peer_manager::connector::PeerManagerConnector;
use crate::network::peer_manager::connector::PeerRemover;
pub use crate::network::peer_manager::notification::{
    PeerManagerNotification, PeerNotificationIter,
};
use crate::network::peer_manager::peer_map::{PeerMap, PeerStatus};
use crate::threading::pacemaker;

use uuid::Uuid;

// the number of retry attempts for an active endpoint before the PeerManager will try other
// endpoints associated with a peer
const DEFAULT_MAXIMUM_RETRY_ATTEMPTS: u64 = 5;
// Default value of how often the Pacemaker should send RetryPending message
const DEFAULT_PACEMAKER_INTERVAL: u64 = 10;
// Default value for maximum time between retrying a peers endpoints
const DEFAULT_MAXIMUM_RETRY_FREQUENCY: u64 = 300;

#[derive(Debug, Clone)]
pub(crate) enum PeerManagerMessage {
    Shutdown,
    Request(PeerManagerRequest),
    Subscribe(Sender<PeerManagerNotification>),
    InternalNotification(ConnectionManagerNotification),
    RetryPending,
}

impl From<ConnectionManagerNotification> for PeerManagerMessage {
    fn from(notification: ConnectionManagerNotification) -> Self {
        PeerManagerMessage::InternalNotification(notification)
    }
}

#[derive(Debug, Clone)]
pub(crate) enum PeerManagerRequest {
    AddPeer {
        peer_id: String,
        endpoints: Vec<String>,
        sender: Sender<Result<PeerRef, PeerRefAddError>>,
    },
    AddUnidentified {
        endpoint: String,
        sender: Sender<Result<(), PeerUnknownAddError>>,
    },
    RemovePeer {
        peer_id: String,
        sender: Sender<Result<(), PeerRefRemoveError>>,
    },
    ListPeers {
        sender: Sender<Result<Vec<String>, PeerListError>>,
    },
    ListUnreferencedPeers {
        sender: Sender<Result<Vec<String>, PeerListError>>,
    },
    ConnectionIds {
        sender: Sender<Result<BiHashMap<String, String>, PeerConnectionIdError>>,
    },
    GetConnectionId {
        peer_id: String,
        sender: Sender<Result<Option<String>, PeerLookupError>>,
    },
    GetPeerId {
        connection_id: String,
        sender: Sender<Result<Option<String>, PeerLookupError>>,
    },
}

/// A PeerRef is used to keep track of peer references. When dropped, the PeerRef will send
/// a request to the PeerManager to remove a reference to the peer, thus removing the peer if no
/// more references exists.

#[derive(Debug, PartialEq)]
pub struct PeerRef {
    peer_id: String,
    peer_remover: PeerRemover,
}

impl PeerRef {
    pub(super) fn new(peer_id: String, peer_remover: PeerRemover) -> Self {
        PeerRef {
            peer_id,
            peer_remover,
        }
    }

    pub fn peer_id(&self) -> &str {
        &self.peer_id
    }
}

impl Drop for PeerRef {
    fn drop(&mut self) {
        match self.peer_remover.remove_peer_ref(&self.peer_id) {
            Ok(_) => (),
            Err(err) => error!(
                "Unable to remove reference to {} on drop: {}",
                self.peer_id, err
            ),
        }
    }
}

/// An entry of unreferenced peers, that may connected externally, but not yet requested locally.
#[derive(Debug)]
struct UnreferencedPeer {
    endpoint: String,
    connection_id: String,
}

/// The PeerManager is in charge of keeping track of peers and their ref count, as well as
/// requesting connections from the ConnectionManager. If a peer has disconnected, the PeerManager
/// will also try the peer's other endpoints until one is successful.
pub struct PeerManager {
    connection_manager_connector: Connector,
    join_handle: Option<thread::JoinHandle<()>>,
    sender: Option<Sender<PeerManagerMessage>>,
    shutdown_handle: Option<ShutdownHandle>,
    max_retry_attempts: Option<u64>,
    retry_interval: u64,
}

impl PeerManager {
    pub fn new(
        connector: Connector,
        max_retry_attempts: Option<u64>,
        retry_interval: Option<u64>,
    ) -> Self {
        let retry_interval = retry_interval.unwrap_or(DEFAULT_PACEMAKER_INTERVAL);
        PeerManager {
            connection_manager_connector: connector,
            join_handle: None,
            sender: None,
            shutdown_handle: None,
            max_retry_attempts,
            retry_interval,
        }
    }

    /// Start the PeerManager
    ///
    /// Returns a PeerManagerConnector that can be used to send requests to the PeerManager.
    pub fn start(&mut self) -> Result<PeerManagerConnector, PeerManagerError> {
        let (sender, recv) = channel();
        if self.sender.is_some() {
            return Err(PeerManagerError::StartUpError(
                "PeerManager has already been started".to_string(),
            ));
        }
        let connector = self.connection_manager_connector.clone();
        let peer_remover = PeerRemover {
            sender: sender.clone(),
        };

        let subscriber_id = connector.subscribe(sender.clone()).map_err(|err| {
            PeerManagerError::StartUpError(format!(
                "Unable to subscribe to connection manager notifications: {}",
                err
            ))
        })?;

        let pacemaker = pacemaker::Pacemaker::builder()
            .with_interval(self.retry_interval)
            .with_sender(sender.clone())
            .with_message_factory(|| PeerManagerMessage::RetryPending)
            .start()
            .map_err(|err| PeerManagerError::StartUpError(err.to_string()))?;

        let pacemaker_shutdown_signaler = pacemaker.shutdown_signaler();

        let max_retry_attempts = self
            .max_retry_attempts
            .unwrap_or(DEFAULT_MAXIMUM_RETRY_ATTEMPTS);
        let join_handle = thread::Builder::new()
            .name("Peer Manager".into())
            .spawn(move || {
                let mut peers = PeerMap::new();
                // a map of identities to unreferenced peers.
                let mut unreferenced_peers = HashMap::new();
                let mut ref_map = RefMap::new();
                let mut subscribers = Vec::new();
                loop {
                    match recv.recv() {
                        Ok(PeerManagerMessage::Shutdown) => break,
                        Ok(PeerManagerMessage::Request(request)) => {
                            handle_request(
                                request,
                                connector.clone(),
                                &mut unreferenced_peers,
                                &mut peers,
                                &peer_remover,
                                &mut ref_map,
                            );
                        }
                        Ok(PeerManagerMessage::Subscribe(sender)) => {
                            subscribers.push(sender);
                        }
                        Ok(PeerManagerMessage::InternalNotification(notification)) => {
                            handle_notifications(
                                notification,
                                &mut unreferenced_peers,
                                &mut peers,
                                connector.clone(),
                                &mut subscribers,
                                max_retry_attempts,
                            )
                        }
                        Ok(PeerManagerMessage::RetryPending) => {
                            retry_pending(&mut peers, connector.clone())
                        }
                        Err(_) => {
                            warn!("All senders have disconnected");
                            break;
                        }
                    }
                }

                if let Err(err) = connector.unsubscribe(subscriber_id) {
                    error!(
                        "Unable to unsubscribe from connection manager notifications: {}",
                        err
                    );
                }

                pacemaker.await_shutdown();
            });

        match join_handle {
            Ok(join_handle) => {
                self.join_handle = Some(join_handle);
            }
            Err(err) => {
                return Err(PeerManagerError::StartUpError(format!(
                    "Unable to start PeerManager thread {}",
                    err
                )))
            }
        }

        self.shutdown_handle = Some(ShutdownHandle {
            sender: sender.clone(),
            pacemaker_shutdown_signaler,
        });
        self.sender = Some(sender.clone());
        Ok(PeerManagerConnector::new(sender))
    }

    pub fn shutdown_handle(&self) -> Option<ShutdownHandle> {
        self.shutdown_handle.clone()
    }

    pub fn await_shutdown(self) {
        let join_handle = if let Some(jh) = self.join_handle {
            jh
        } else {
            return;
        };

        if let Err(err) = join_handle.join() {
            error!("Peer manager thread did not shutdown correctly: {:?}", err);
        }
    }

    pub fn shutdown_and_wait(self) {
        if let Some(sh) = self.shutdown_handle.clone() {
            sh.shutdown();
        } else {
            return;
        }

        self.await_shutdown();
    }
}

#[derive(Clone)]
pub struct ShutdownHandle {
    sender: Sender<PeerManagerMessage>,
    pacemaker_shutdown_signaler: pacemaker::ShutdownSignaler,
}

impl ShutdownHandle {
    pub fn shutdown(&self) {
        self.pacemaker_shutdown_signaler.shutdown();
        if self.sender.send(PeerManagerMessage::Shutdown).is_err() {
            warn!("PeerManager is no longer running");
        }
    }
}

fn handle_request(
    request: PeerManagerRequest,
    connector: Connector,
    unreferenced_peers: &mut HashMap<String, UnreferencedPeer>,
    peers: &mut PeerMap,
    peer_remover: &PeerRemover,
    ref_map: &mut RefMap,
) {
    match request {
        PeerManagerRequest::AddPeer {
            peer_id,
            endpoints,
            sender,
        } => {
            if sender
                .send(add_peer(
                    peer_id,
                    endpoints,
                    connector,
                    unreferenced_peers,
                    peers,
                    peer_remover,
                    ref_map,
                ))
                .is_err()
            {
                warn!("connector dropped before receiving result of adding peer");
            }
        }
        PeerManagerRequest::AddUnidentified { endpoint, sender } => {
            if sender.send(add_unidentified(endpoint, connector)).is_err() {
                warn!("connector dropped before receiving result of adding unidentified peer");
            }
        }
        PeerManagerRequest::RemovePeer { peer_id, sender } => {
            if sender
                .send(remove_peer(
                    peer_id,
                    connector,
                    unreferenced_peers,
                    peers,
                    ref_map,
                ))
                .is_err()
            {
                warn!("connector dropped before receiving result of removing peer");
            }
        }
        PeerManagerRequest::ListPeers { sender } => {
            if sender.send(Ok(peers.peer_ids())).is_err() {
                warn!("connector dropped before receiving result of list peers");
            }
        }

        PeerManagerRequest::ListUnreferencedPeers { sender } => {
            let peer_ids = unreferenced_peers.keys().map(|s| s.to_owned()).collect();
            if sender.send(Ok(peer_ids)).is_err() {
                warn!("connector dropped before receiving result of list unreferenced peers");
            }
        }
        PeerManagerRequest::ConnectionIds { sender } => {
            if sender.send(Ok(peers.connection_ids())).is_err() {
                warn!("connector dropped before receiving result of connection ids");
            }
        }
        PeerManagerRequest::GetConnectionId { peer_id, sender } => {
            let connection_id = peers
                .get_by_peer_id(&peer_id)
                .map(|meta| meta.connection_id.clone())
                .or_else(|| {
                    unreferenced_peers
                        .get(&peer_id)
                        .map(|meta| meta.connection_id.clone())
                });

            if sender.send(Ok(connection_id)).is_err() {
                warn!("connector dropped before receiving result of get connection id");
            }
        }
        PeerManagerRequest::GetPeerId {
            connection_id,
            sender,
        } => {
            let peer_id = peers
                .get_by_connection_id(&connection_id)
                .map(|meta| meta.id.clone())
                .or_else(|| {
                    unreferenced_peers
                        .iter()
                        .find(|(_, meta)| meta.connection_id == connection_id)
                        .map(|(peer_id, _)| peer_id.clone())
                });

            if sender.send(Ok(peer_id)).is_err() {
                warn!("connector dropped before receiving result of get peer id");
            }
        }
    };
}

fn add_peer(
    peer_id: String,
    endpoints: Vec<String>,
    connector: Connector,
    unreferenced_peers: &mut HashMap<String, UnreferencedPeer>,
    peers: &mut PeerMap,
    peer_remover: &PeerRemover,
    ref_map: &mut RefMap,
) -> Result<PeerRef, PeerRefAddError> {
    let new_ref_count = ref_map.add_ref(peer_id.to_string());

    // if this is not a new peer, return success
    if new_ref_count > 1 {
        let peer_ref = PeerRef::new(peer_id, peer_remover.clone());
        return Ok(peer_ref);
    };

    // if it is a unreferenced peer, promote it to a fully-referenced peer
    if let Some(UnreferencedPeer {
        connection_id,
        endpoint,
    }) = unreferenced_peers.remove(&peer_id)
    {
        peers.insert(
            peer_id.clone(),
            connection_id,
            endpoints,
            endpoint,
            PeerStatus::Connected,
        );

        let peer_ref = PeerRef::new(peer_id, peer_remover.clone());
        return Ok(peer_ref);
    }

    debug!("Attempting to peer with {}", peer_id);
    let connection_id = format!("{}", Uuid::new_v4());

    let mut active_endpoint = match endpoints.get(0) {
        Some(endpoint) => endpoint.to_string(),
        None => {
            return Err(PeerRefAddError::AddError(format!(
                "No endpoints provided for peer {}",
                peer_id
            )))
        }
    };

    for endpoint in endpoints.iter() {
        match connector.request_connection(&endpoint, &connection_id) {
            Ok(()) => {
                active_endpoint = endpoint.to_string();
                break;
            }
            // If the request_connection errored we will retry in the future
            Err(err) => {
                error!("Unable to request connection for peer {}: {}", peer_id, err);
            }
        }
    }

    peers.insert(
        peer_id.clone(),
        connection_id,
        endpoints.to_vec(),
        active_endpoint,
        PeerStatus::Pending,
    );
    let peer_ref = PeerRef::new(peer_id, peer_remover.clone());
    Ok(peer_ref)
}

// Request a connection, the resulting connection will be treated as an InboundConnection
fn add_unidentified(endpoint: String, connector: Connector) -> Result<(), PeerUnknownAddError> {
    debug!("Attempting to peer with unidentified peer");
    let connection_id = format!("{}", Uuid::new_v4());
    match connector.request_connection(&endpoint, &connection_id) {
        Ok(()) => Ok(()),
        Err(err) => {
            warn!("Unable to peer with unidentified peer: {}", endpoint);
            // unable to connect to any of the endpoints provided
            Err(PeerUnknownAddError::AddError(format!(
                "Unable to connect to endpoint {} that was provided for unidentified peer: {}",
                endpoint, err
            )))
        }
    }
}

fn remove_peer(
    peer_id: String,
    connector: Connector,
    unreferenced_peers: &mut HashMap<String, UnreferencedPeer>,
    peers: &mut PeerMap,
    ref_map: &mut RefMap,
) -> Result<(), PeerRefRemoveError> {
    debug!("Removing peer: {}", peer_id);

    // remove from the unreferenced peers, if it is there.
    unreferenced_peers.remove(&peer_id);

    // remove the reference
    let removed_peer = ref_map.remove_ref(&peer_id);
    if let Some(removed_peer) = removed_peer {
        let peer_metadata = peers.remove(&removed_peer).ok_or_else(|| {
            PeerRefRemoveError::RemoveError(format!(
                "Peer {} has already been removed from the peer map",
                peer_id
            ))
        })?;

        // If the peer is pending or invalid there is no connection to remove
        if peer_metadata.status == PeerStatus::Pending {
            return Ok(());
        }
        match connector.remove_connection(&peer_metadata.active_endpoint) {
            Ok(Some(_)) => {
                debug!(
                    "Peer {} has been removed and connection {} has been closed",
                    peer_id, peer_metadata.active_endpoint
                );
                Ok(())
            }
            Ok(None) => Err(PeerRefRemoveError::RemoveError(
                "No connection to remove, something has gone wrong".to_string(),
            )),
            Err(err) => Err(PeerRefRemoveError::RemoveError(format!("{}", err))),
        }
    } else {
        // if the peer has not been fully removed, return OK
        Ok(())
    }
}

fn handle_notifications(
    notification: ConnectionManagerNotification,
    unreferenced_peers: &mut HashMap<String, UnreferencedPeer>,
    peers: &mut PeerMap,
    connector: Connector,
    subscribers: &mut Vec<Sender<PeerManagerNotification>>,
    max_retry_attempts: u64,
) {
    match notification {
        // If a connection has disconnected, forward notification to subscribers
        ConnectionManagerNotification::Disconnected { endpoint } => {
            if let Some(mut peer_metadata) = peers.get_peer_from_endpoint(&endpoint).cloned() {
                let notification = PeerManagerNotification::Disconnected {
                    peer: peer_metadata.id.to_string(),
                };
                subscribers.retain(|sender| sender.send(notification.clone()).is_ok());
                // set peer to disconnected
                peer_metadata.status = PeerStatus::Disconnected { retry_attempts: 1 };
                if let Err(err) = peers.update_peer(peer_metadata) {
                    error!("Unable to update peer: {}", err);
                }
            }
        }
        ConnectionManagerNotification::NonFatalConnectionError { endpoint, attempts } => {
            // Check if the disconnected peer has reached the retry limit, if so try to find a
            // different endpoint that can be connected to
            if let Some(mut peer_metadata) = peers.get_peer_from_endpoint(&endpoint).cloned() {
                if attempts >= max_retry_attempts {
                    for endpoint in peer_metadata.endpoints.iter() {
                        // do not retry the connection that is currently failing
                        if endpoint == &peer_metadata.active_endpoint {
                            continue;
                        }
                        match connector.request_connection(&endpoint, &peer_metadata.connection_id)
                        {
                            Ok(()) => break,
                            Err(err) => error!(
                                "Unable to request connection for peer {} at endpoint {}: {}",
                                peer_metadata.id, endpoint, err
                            ),
                        }
                    }

                    peer_metadata.status = PeerStatus::Disconnected {
                        retry_attempts: attempts,
                    };

                    if let Err(err) = peers.update_peer(peer_metadata) {
                        error!("Unable to update peer: {}", err);
                    }
                }
            }
        }
        ConnectionManagerNotification::InboundConnection {
            endpoint,
            connection_id,
            identity,
        } => {
            info!(
                "Received peer connection from {} (remote endpoint: {})",
                identity, endpoint
            );

            // If we got an inbound counnection for an existing peer, replace old connection with
            // this new one.
            if let Some(mut peer_metadata) = peers.get_by_peer_id(&identity).cloned() {
                peer_metadata.status = PeerStatus::Connected;
                peer_metadata.connection_id = connection_id;

                if let Err(err) = connector.remove_connection(&peer_metadata.active_endpoint) {
                    error!("Unable to clean up old connection: {}", err);
                }
                let notification = PeerManagerNotification::Connected {
                    peer: peer_metadata.id.to_string(),
                };
                subscribers.retain(|sender| sender.send(notification.clone()).is_ok());

                peer_metadata.active_endpoint = endpoint;
                if let Err(err) = peers.update_peer(peer_metadata) {
                    error!("Unable to update peer: {}", err);
                }
            } else {
                unreferenced_peers.insert(
                    identity,
                    UnreferencedPeer {
                        connection_id,
                        endpoint,
                    },
                );
            }
        }
        ConnectionManagerNotification::Connected {
            endpoint,
            identity,
            connection_id,
        } => handle_connected(
            endpoint,
            identity,
            connection_id,
            unreferenced_peers,
            peers,
            connector,
            subscribers,
        ),
        ConnectionManagerNotification::FatalConnectionError { endpoint, error } => {
            handle_fatal_connection(endpoint, error.to_string(), peers, subscribers)
        }
    }
}

fn handle_connected(
    endpoint: String,
    identity: String,
    connection_id: String,
    unreferenced_peers: &mut HashMap<String, UnreferencedPeer>,
    peers: &mut PeerMap,
    connector: Connector,
    subscribers: &mut Vec<Sender<PeerManagerNotification>>,
) {
    if let Some(mut peer_metadata) = peers.get_peer_from_endpoint(&endpoint).cloned() {
        match peer_metadata.status {
            PeerStatus::Connected => {
                let notification = PeerManagerNotification::Connected {
                    peer: peer_metadata.id.to_string(),
                };
                subscribers.retain(|sender| sender.send(notification.clone()).is_ok());
            }
            PeerStatus::Pending => {
                if identity != peer_metadata.id {
                    if let Err(err) = connector.remove_connection(&endpoint) {
                        error!("Unable to clean up mismatched identity connection: {}", err);
                    }

                    // tell subscribers this Peer is currently disconnected
                    let notification = PeerManagerNotification::Disconnected {
                        peer: peer_metadata.id.to_string(),
                    };

                    subscribers.retain(|sender| sender.send(notification.clone()).is_ok());
                    error!(
                        "Peer {} (via {}) presented a mismatched identity {}",
                        identity, endpoint, peer_metadata.id
                    );

                    // set its status to pending, this will cause the endpoints to be retried at
                    // a later time
                    peer_metadata.status = PeerStatus::Pending;
                    if let Err(err) = peers.update_peer(peer_metadata) {
                        error!("Unable to update peer: {}", err);
                    }
                    return;
                }

                peer_metadata.status = PeerStatus::Connected;

                let notification = PeerManagerNotification::Connected {
                    peer: peer_metadata.id.to_string(),
                };

                debug!("Peer {} connected via {}", peer_metadata.id, endpoint);
                if let Err(err) = peers.update_peer(peer_metadata) {
                    error!("Unable to update peer: {}", err);
                }

                subscribers.retain(|sender| sender.send(notification.clone()).is_ok());
            }
            PeerStatus::Disconnected { .. } => {
                // remove old connection if it has been replaced
                if endpoint != peer_metadata.active_endpoint {
                    if let Err(err) = connector.remove_connection(&peer_metadata.active_endpoint) {
                        error!(
                            "Unable to remove connection for {}: {}",
                            peer_metadata.active_endpoint, err
                        );
                    }
                }

                if identity != peer_metadata.id {
                    if let Err(err) = connector.remove_connection(&endpoint) {
                        error!("Unable to clean up mismatched identity connection: {}", err);
                    }

                    // tell subscribers this Peer is currently disconnected
                    let notification = PeerManagerNotification::Disconnected {
                        peer: peer_metadata.id.to_string(),
                    };

                    subscribers.retain(|sender| sender.send(notification.clone()).is_ok());
                    error!(
                        "Peer {} (via {}) presented a mismatched identity {}",
                        identity, endpoint, peer_metadata.id
                    );

                    // reset retry settings
                    peer_metadata.retry_frequency = min(
                        peer_metadata.retry_frequency * 2,
                        DEFAULT_MAXIMUM_RETRY_FREQUENCY,
                    );
                    peer_metadata.last_connection_attempt = Instant::now();
                    // set its status to pending, this will cause the endpoints to be retried at
                    // a later time
                    peer_metadata.status = PeerStatus::Pending;
                    if let Err(err) = peers.update_peer(peer_metadata) {
                        error!("Unable to update peer: {}", err);
                    }
                    return;
                }

                peer_metadata.status = PeerStatus::Connected;
                peer_metadata.active_endpoint = endpoint.clone();
                let notification = PeerManagerNotification::Connected {
                    peer: peer_metadata.id.to_string(),
                };

                debug!("Peer {} connected via {}", peer_metadata.id, endpoint);
                if let Err(err) = peers.update_peer(peer_metadata) {
                    error!("Unable to update peer: {}", err);
                }

                subscribers.retain(|sender| sender.send(notification.clone()).is_ok());
            }
        }
    } else {
        // Treat unknown peer as unreferenced
        unreferenced_peers.insert(
            identity,
            UnreferencedPeer {
                connection_id,
                endpoint,
            },
        );
    }
}

fn handle_fatal_connection(
    endpoint: String,
    error: String,
    peers: &mut PeerMap,
    subscribers: &mut Vec<Sender<PeerManagerNotification>>,
) {
    if let Some(mut peer_metadata) = peers.get_peer_from_endpoint(&endpoint).cloned() {
        warn!(
            "Peer {} is invalid: {}",
            peer_metadata.id.to_string(),
            error
        );

        // Tell subscribers this peer is disconnected
        let notification = PeerManagerNotification::Disconnected {
            peer: peer_metadata.id.to_string(),
        };

        // reset retry settings
        peer_metadata.retry_frequency = min(
            peer_metadata.retry_frequency * 2,
            DEFAULT_MAXIMUM_RETRY_FREQUENCY,
        );
        peer_metadata.last_connection_attempt = Instant::now();

        // set peer to pending so its endpoints will be retried in the future
        peer_metadata.status = PeerStatus::Pending;
        if let Err(err) = peers.update_peer(peer_metadata) {
            error!("Unable to update peer: {}", err);
        }

        subscribers.retain(|sender| sender.send(notification.clone()).is_ok());
    }
}

// If a pending peers retry retry_frequency has elapsed, retry their endpoints. If successful,
// their active endpoint will be updated. The retry_frequency will be increased and
// and last_connection_attempt reset.
fn retry_pending(peers: &mut PeerMap, connector: Connector) {
    let mut to_retry = Vec::new();
    for (_, peer) in peers.get_pending() {
        if peer.last_connection_attempt.elapsed().as_secs() > peer.retry_frequency {
            to_retry.push(peer.clone());
        }
    }

    for mut peer_metadata in to_retry {
        for endpoint in peer_metadata.endpoints.iter() {
            match connector.request_connection(&endpoint, &peer_metadata.connection_id) {
                Ok(()) => peer_metadata.active_endpoint = endpoint.to_string(),
                // If request_connection errored we will retry in the future
                Err(err) => {
                    error!(
                        "Unable to request connection for peer {}: {}",
                        peer_metadata.id, err
                    );
                }
            }
        }

        peer_metadata.retry_frequency = min(
            peer_metadata.retry_frequency * 2,
            DEFAULT_MAXIMUM_RETRY_FREQUENCY,
        );
        peer_metadata.last_connection_attempt = Instant::now();
        if let Err(err) = peers.update_peer(peer_metadata) {
            error!("Unable to update peer: {}", err);
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use std::collections::VecDeque;
    use std::sync::mpsc;

    use crate::mesh::Mesh;
    use crate::network::connection_manager::{
        AuthorizationResult, Authorizer, AuthorizerError, ConnectionManager,
    };
    use crate::protos::network::{NetworkMessage, NetworkMessageType};
    use crate::transport::inproc::InprocTransport;
    use crate::transport::raw::RawTransport;
    use crate::transport::{Connection, Transport};

    // Test that a call to add_peer_ref returns the correct PeerRef
    //
    // 1. add test_peer
    // 2. verify that the returned PeerRef contains the test_peer id
    // 3. verify the the a Connected notification is received
    #[test]
    fn test_peer_manager_add_peer() {
        let mut transport = Box::new(InprocTransport::default());
        let mut listener = transport.listen("inproc://test").unwrap();

        thread::spawn(move || {
            listener.accept().unwrap();
        });

        let mesh = Mesh::new(512, 128);
        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(NoopAuthorizer::new("test_peer")))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(transport.clone())
            .start()
            .expect("Unable to start Connection Manager");

        let connector = cm.connector();
        let mut peer_manager = PeerManager::new(connector, None, Some(1));
        let peer_connector = peer_manager.start().expect("Cannot start peer_manager");
        let mut subscriber = peer_connector
            .subscribe()
            .expect("Unable to get subscriber");
        let peer_ref = peer_connector
            .add_peer_ref("test_peer".to_string(), vec!["inproc://test".to_string()])
            .expect("Unable to add peer");

        assert_eq!(peer_ref.peer_id, "test_peer");

        let notification = subscriber.next().expect("Unable to get new notifications");
        assert!(
            notification
                == PeerManagerNotification::Connected {
                    peer: "test_peer".to_string(),
                }
        );

        peer_manager.shutdown_handle().unwrap().shutdown();
        cm.shutdown_signaler().shutdown();
        peer_manager.await_shutdown();
        cm.await_shutdown();
        mesh.shutdown_signaler().shutdown();
    }

    // Test that a call to add_peer_ref, where the authorizer returns an different id than
    // requested, the connector returns an error.
    //
    // 1. add test_peer, whose identity is different_peer
    // 2. verify that an AddPeer returns succesfully
    // 4. validate a Disconnected notfication is returned,
    // 5. drop peer ref
    // 6. verify that the connection is removed.
    #[test]
    fn test_peer_manager_add_peer_identity_mismatch() {
        let mut transport = Box::new(InprocTransport::default());
        let mut listener = transport.listen("inproc://test").unwrap();

        thread::spawn(move || {
            listener.accept().unwrap();
        });

        let mesh = Mesh::new(512, 128);
        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(NoopAuthorizer::new("different_peer")))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(transport.clone())
            .start()
            .expect("Unable to start Connection Manager");

        let connector = cm.connector();
        let mut peer_manager = PeerManager::new(connector.clone(), None, Some(1));

        let peer_connector = peer_manager.start().expect("Cannot start peer_manager");
        let mut subscriber = peer_connector
            .subscribe()
            .expect("Unable to get subscriber");

        let peer_ref = peer_connector
            .add_peer_ref("test_peer".to_string(), vec!["inproc://test".to_string()])
            .expect("Unable to add peer");

        assert_eq!(peer_ref.peer_id, "test_peer");

        let notification = subscriber.next().expect("Unable to get new notifications");
        assert!(
            notification
                == PeerManagerNotification::Disconnected {
                    peer: "test_peer".to_string(),
                }
        );

        drop(peer_ref);

        assert!(connector
            .list_connections()
            .expect("Unable to list connections")
            .is_empty());

        peer_manager.shutdown_handle().unwrap().shutdown();
        cm.shutdown_signaler().shutdown();
        peer_manager.await_shutdown();
        cm.await_shutdown();
        mesh.shutdown_signaler().shutdown();
    }

    // Test that a call to add_peer_ref with a peer with multiple endpoints is successful, even if
    // the first endpoint is not available
    //
    // 1. add test_peer with two endpoints. The first endpoint will fail and cause the peer
    //    manager to try the second
    // 2. verify that the returned PeerRef contains the test_peer id
    // 3. verify the the a Connected notification is received
    #[test]
    fn test_peer_manager_add_peer_endpoints() {
        let mut transport = Box::new(InprocTransport::default());
        let mut listener = transport.listen("inproc://test").unwrap();

        thread::spawn(move || {
            listener.accept().unwrap();
        });

        let mesh = Mesh::new(512, 128);
        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(NoopAuthorizer::new("test_peer")))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(transport.clone())
            .start()
            .expect("Unable to start Connection Manager");

        let connector = cm.connector();
        let mut peer_manager = PeerManager::new(connector.clone(), None, Some(1));

        let peer_connector = peer_manager.start().expect("Cannot start peer_manager");
        let mut subscriber = peer_connector
            .subscribe()
            .expect("Unable to get subscriber");
        let peer_ref = peer_connector
            .add_peer_ref(
                "test_peer".to_string(),
                vec![
                    "inproc://bad_endpoint".to_string(),
                    "inproc://test".to_string(),
                ],
            )
            .expect("Unable to add peer");

        assert_eq!(peer_ref.peer_id, "test_peer");

        let notification = subscriber.next().expect("Unable to get new notifications");
        assert!(
            notification
                == PeerManagerNotification::Connected {
                    peer: "test_peer".to_string(),
                }
        );

        peer_manager.shutdown_handle().unwrap().shutdown();
        cm.shutdown_signaler().shutdown();
        peer_manager.await_shutdown();
        cm.await_shutdown();
        mesh.shutdown_signaler().shutdown();
    }

    // Test that the same peer can be added multiple times.
    //
    // 1. add test_peer
    // 2. verify the the a Connected notification is received
    // 3. add the same peer, and see it is successful
    #[test]
    fn test_peer_manager_add_peer_multiple_times() {
        let mut transport = Box::new(InprocTransport::default());
        let mut listener = transport.listen("inproc://test").unwrap();

        thread::spawn(move || {
            listener.accept().unwrap();
        });

        let mesh = Mesh::new(512, 128);
        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(NoopAuthorizer::new("test_peer")))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(transport.clone())
            .start()
            .expect("Unable to start Connection Manager");

        let connector = cm.connector();
        let mut peer_manager = PeerManager::new(connector, None, Some(1));
        let peer_connector = peer_manager.start().expect("Cannot start peer_manager");
        let mut subscriber = peer_connector
            .subscribe()
            .expect("Unable to get subscriber");
        let peer_ref = peer_connector
            .add_peer_ref("test_peer".to_string(), vec!["inproc://test".to_string()])
            .expect("Unable to add peer");

        assert_eq!(peer_ref.peer_id, "test_peer");

        let notification = subscriber.next().expect("Unable to get new notifications");
        assert!(
            notification
                == PeerManagerNotification::Connected {
                    peer: "test_peer".to_string(),
                }
        );

        let peer_ref = peer_connector
            .add_peer_ref("test_peer".to_string(), vec!["inproc://test".to_string()])
            .expect("Unable to add peer");

        assert_eq!(peer_ref.peer_id, "test_peer");

        peer_manager.shutdown_handle().unwrap().shutdown();
        cm.shutdown_signaler().shutdown();
        peer_manager.await_shutdown();
        cm.await_shutdown();
        mesh.shutdown_signaler().shutdown();
    }

    // Test that list_peer returns the correct list of peers
    //
    // 1. add test_peer
    // 2. verify the the a Connected notification is received
    // 3. add next_peer
    // 4. verify the the a Connected notification is received
    // 5. call list_peers
    // 6. verify that the sorted list of peers contains both test_peer and next_peer
    #[test]
    fn test_peer_manager_list_peer() {
        let mut transport = Box::new(InprocTransport::default());
        let mut listener = transport.listen("inproc://test").unwrap();

        thread::spawn(move || {
            listener.accept().unwrap();
        });

        let mut listener = transport.listen("inproc://test_2").unwrap();
        thread::spawn(move || {
            listener.accept().unwrap();
        });

        let mesh = Mesh::new(512, 128);
        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(NoopAuthorizer::new_multiple(&[
                "test_peer",
                "next_peer",
            ])))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(transport.clone())
            .start()
            .expect("Unable to start Connection Manager");

        let connector = cm.connector();
        let mut peer_manager = PeerManager::new(connector, None, Some(1));
        let peer_connector = peer_manager.start().expect("Cannot start peer_manager");
        let mut subscriber = peer_connector
            .subscribe()
            .expect("Unable to get subscriber");
        let peer_ref_1 = peer_connector
            .add_peer_ref("test_peer".to_string(), vec!["inproc://test".to_string()])
            .expect("Unable to add peer");

        assert_eq!(peer_ref_1.peer_id, "test_peer");

        let notification = subscriber.next().expect("Unable to get new notifications");
        assert!(
            notification
                == PeerManagerNotification::Connected {
                    peer: "test_peer".to_string(),
                }
        );

        let peer_ref_2 = peer_connector
            .add_peer_ref("next_peer".to_string(), vec!["inproc://test_2".to_string()])
            .expect("Unable to add peer");

        assert_eq!(peer_ref_2.peer_id, "next_peer");

        let notification = subscriber.next().expect("Unable to get new notifications");
        assert!(
            notification
                == PeerManagerNotification::Connected {
                    peer: "next_peer".to_string(),
                }
        );

        let mut peer_list = peer_connector
            .list_peers()
            .expect("Unable to get peer list");

        peer_list.sort();

        assert_eq!(
            peer_list,
            vec!["next_peer".to_string(), "test_peer".to_string()]
        );

        peer_manager.shutdown_handle().unwrap().shutdown();
        cm.shutdown_signaler().shutdown();
        peer_manager.await_shutdown();
        cm.await_shutdown();
        mesh.shutdown_signaler().shutdown();
    }

    // Test that list_peer returns the correct list of peers
    //
    // 1. add test_peer
    // 2. add next_peer
    // 3. call connection_ids
    // 4. verify that the sorted map contains both test_peer and next_peer
    #[test]
    fn test_peer_manager_connection_ids() {
        let mut transport = Box::new(InprocTransport::default());
        let mut listener = transport.listen("inproc://test").unwrap();

        thread::spawn(move || {
            listener.accept().unwrap();
        });

        let mut listener = transport.listen("inproc://test_2").unwrap();
        thread::spawn(move || {
            listener.accept().unwrap();
        });

        let mesh = Mesh::new(512, 128);
        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(NoopAuthorizer::new_multiple(&[
                "test_peer",
                "next_peer",
            ])))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(transport.clone())
            .start()
            .expect("Unable to start Connection Manager");

        let connector = cm.connector();
        let mut peer_manager = PeerManager::new(connector, None, Some(1));
        let peer_connector = peer_manager.start().expect("Cannot start peer_manager");
        let mut subscriber = peer_connector
            .subscribe()
            .expect("Unable to get subscriber");
        let peer_ref_1 = peer_connector
            .add_peer_ref("test_peer".to_string(), vec!["inproc://test".to_string()])
            .expect("Unable to add peer");

        assert_eq!(peer_ref_1.peer_id, "test_peer");

        let notification = subscriber.next().expect("Unable to get new notifications");
        assert!(
            notification
                == PeerManagerNotification::Connected {
                    peer: "test_peer".to_string(),
                }
        );

        let peer_ref_2 = peer_connector
            .add_peer_ref("next_peer".to_string(), vec!["inproc://test_2".to_string()])
            .expect("Unable to add peer");

        assert_eq!(peer_ref_2.peer_id, "next_peer");

        let notification = subscriber.next().expect("Unable to get new notifications");
        assert!(
            notification
                == PeerManagerNotification::Connected {
                    peer: "next_peer".to_string(),
                }
        );

        let peers = peer_connector
            .connection_ids()
            .expect("Unable to get peer list");

        assert!(peers.get_by_key("next_peer").is_some());

        assert!(peers.get_by_key("test_peer").is_some());

        peer_manager.shutdown_handle().unwrap().shutdown();
        cm.shutdown_signaler().shutdown();
        peer_manager.await_shutdown();
        cm.await_shutdown();
        mesh.shutdown_signaler().shutdown();
    }

    // Test that when a PeerRef is dropped, a remove peer request is properly sent and the peer
    // is removed
    //
    // 1. add test_peer
    // 2. call list peers
    // 3. verify that the peer list contains test_peer
    // 4. drop the PeerRef
    // 5. call list peers
    // 6. verify that the new peer list is empty
    #[test]
    fn test_peer_manager_drop_peer_ref() {
        let mut transport = Box::new(InprocTransport::default());
        let mut listener = transport.listen("inproc://test").unwrap();

        thread::spawn(move || {
            listener.accept().unwrap();
        });

        let mesh = Mesh::new(512, 128);
        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(NoopAuthorizer::new("test_peer")))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(transport.clone())
            .start()
            .expect("Unable to start Connection Manager");

        let connector = cm.connector();
        let mut peer_manager = PeerManager::new(connector, None, Some(1));

        let peer_connector = peer_manager.start().expect("Cannot start peer_manager");

        {
            let mut subscriber = peer_connector
                .subscribe()
                .expect("Unable to get subscriber");
            let peer_ref = peer_connector
                .add_peer_ref("test_peer".to_string(), vec!["inproc://test".to_string()])
                .expect("Unable to add peer");

            assert_eq!(peer_ref.peer_id, "test_peer");
            let notification = subscriber.next().expect("Unable to get new notifications");
            assert!(
                notification
                    == PeerManagerNotification::Connected {
                        peer: "test_peer".to_string(),
                    }
            );

            let peer_list = peer_connector
                .list_peers()
                .expect("Unable to get peer list");

            assert_eq!(peer_list, vec!["test_peer".to_string()]);
        }
        // drop peer_ref

        let peer_list = peer_connector
            .list_peers()
            .expect("Unable to get peer list");

        assert_eq!(peer_list, Vec::<String>::new());

        peer_manager.shutdown_handle().unwrap().shutdown();
        cm.shutdown_signaler().shutdown();
        peer_manager.await_shutdown();
        cm.await_shutdown();
        mesh.shutdown_signaler().shutdown();
    }

    // Test that if a peer's endpoint disconnects and does not reconnect during a set timeout, the
    // PeerManager will retry the peers list of endpoints trying to find an endpoint that is
    // available.
    //
    // 1. add test_peer, this will connected to the first endpoint
    // 2. verify that the test_peer connection receives a heartbeat
    // 3. disconnect the connection made to test_peer
    // 4. verify that subscribers will receive a Disconnected notification
    // 5. drop the listener for the first endpoint to force the attempt on the second endpoint
    // 6. verify that subscribers will receive a Connected notfication when the new active endpoint
    //    is successfully connected to.
    #[test]
    fn test_peer_manager_update_active_endpoint() {
        let mut transport = Box::new(RawTransport::default());
        let mut listener = transport
            .listen("tcp://localhost:0")
            .expect("Cannot listen for connections");
        let endpoint = listener.endpoint();
        let mesh1 = Mesh::new(512, 128);
        let mesh2 = Mesh::new(512, 128);

        let mut listener2 = transport
            .listen("tcp://localhost:0")
            .expect("Cannot listen for connections");
        let endpoint2 = listener2.endpoint();

        let (tx, rx) = mpsc::channel();
        let jh = thread::spawn(move || {
            // accept incoming connection and add it to mesh2
            let conn = listener.accept().expect("Cannot accept connection");
            mesh2
                .add(conn, "test_id".to_string())
                .expect("Cannot add connection to mesh");
            // Verify mesh received heartbeat
            let envelope = mesh2.recv().expect("Cannot receive message");
            let heartbeat: NetworkMessage = protobuf::parse_from_bytes(&envelope.payload())
                .expect("Cannot parse NetworkMessage");
            assert_eq!(
                heartbeat.get_message_type(),
                NetworkMessageType::NETWORK_HEARTBEAT
            );
            // remove connection to cause reconnection attempt
            let mut connection = mesh2
                .remove("test_id")
                .expect("Cannot remove connection from mesh");
            connection
                .disconnect()
                .expect("Connection failed to disconnect");
            // force drop of first listener
            drop(listener);
            // wait for the peer manager to switch endpoints
            let conn = listener2.accept().expect("Unable to accept connection");
            mesh2
                .add(conn, "test_id".to_string())
                .expect("Cannot add connection to mesh");

            rx.recv().unwrap();

            mesh2.shutdown_signaler().shutdown();
        });

        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(NoopAuthorizer::new_multiple(&[
                "test_peer",
                "test_peer",
            ])))
            .with_matrix_life_cycle(mesh1.get_life_cycle())
            .with_matrix_sender(mesh1.get_sender())
            .with_transport(transport)
            .start()
            .expect("Unable to start Connection Manager");

        let connector = cm.connector();
        let mut peer_manager = PeerManager::new(connector, Some(1), Some(1));
        let peer_connector = peer_manager.start().expect("Cannot start peer_manager");
        let mut subscriber = peer_connector.subscribe().expect("Unable to subscribe");
        let peer_ref = peer_connector
            .add_peer_ref("test_peer".to_string(), vec![endpoint, endpoint2])
            .expect("Unable to add peer");

        assert_eq!(peer_ref.peer_id, "test_peer");

        let notification = subscriber.next().expect("Unable to get new notifications");
        assert!(
            notification
                == PeerManagerNotification::Connected {
                    peer: "test_peer".to_string(),
                }
        );

        // receive reconnecting attempt
        let disconnected_notification = subscriber
            .next()
            .expect("Cannot get message from subscriber");
        assert!(
            disconnected_notification
                == PeerManagerNotification::Disconnected {
                    peer: "test_peer".to_string(),
                }
        );

        // receive notifications that the peer is connected to new endpoint
        let connected_notification = subscriber
            .next()
            .expect("Cannot get message from subscriber");

        assert!(
            connected_notification
                == PeerManagerNotification::Connected {
                    peer: "test_peer".to_string(),
                }
        );

        tx.send(()).unwrap();

        jh.join().unwrap();
        peer_manager.shutdown_handle().unwrap().shutdown();
        cm.shutdown_signaler().shutdown();
        peer_manager.await_shutdown();
        cm.await_shutdown();
        mesh1.shutdown_signaler().shutdown();
    }

    // Test that the PeerManager can be started and stopped
    #[test]
    fn test_peer_manager_shutdown() {
        let transport = Box::new(InprocTransport::default());

        let mesh = Mesh::new(512, 128);
        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(NoopAuthorizer::new("test_peer")))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(transport.clone())
            .start()
            .expect("Unable to start Connection Manager");

        let connector = cm.connector();
        let mut peer_manager = PeerManager::new(connector, Some(1), Some(1));
        peer_manager.start().expect("Cannot start peer_manager");

        peer_manager.shutdown_handle().unwrap().shutdown();
        cm.shutdown_signaler().shutdown();
        peer_manager.await_shutdown();
        cm.await_shutdown();
        mesh.shutdown_signaler().shutdown();
    }

    // Test that the PeerManager can receive incoming peer requests and handle them appropriately.
    //
    // 1. Add a connection
    // 2. Verify that it has been added as a unreferenced peer
    // 3. Verify that it can be promoted to a proper peer
    #[test]
    fn test_incoming_peer_request() {
        let mut transport = InprocTransport::default();
        let mut listener = transport.listen("inproc://test").unwrap();

        let mesh = Mesh::new(512, 128);
        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(NoopAuthorizer::new("test_peer")))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(Box::new(transport.clone()))
            .start()
            .expect("Unable to start Connection Manager");

        let connector = cm.connector();

        let recv_connector = connector.clone();
        let jh = thread::spawn(move || {
            let connection = listener.accept().unwrap();
            let (subs_tx, subs_rx): (mpsc::Sender<ConnectionManagerNotification>, _) =
                mpsc::channel();
            let _ = recv_connector
                .subscribe(subs_tx)
                .expect("unable to get subscriber");
            recv_connector.add_inbound_connection(connection).unwrap();
            // wait for inbound connection notfication to come
            subs_rx.recv().expect("unable to get notfication");
        });

        let mut peer_manager = PeerManager::new(connector, Some(1), Some(1));
        let peer_connector = peer_manager.start().expect("Cannot start peer_manager");

        let _conn = transport.connect("inproc://test").unwrap();

        jh.join().unwrap();

        // The peer is not part of the set of active peers
        assert!(peer_connector.list_peers().unwrap().is_empty());

        assert_eq!(
            vec!["test_peer".to_string()],
            peer_connector.list_unreferenced_peers().unwrap()
        );

        let peer_ref = peer_connector
            .add_peer_ref("test_peer".to_string(), vec!["inproc://test".to_string()])
            .expect("Unable to add peer");

        assert_eq!(peer_ref.peer_id, "test_peer");

        let peer_list = peer_connector
            .list_peers()
            .expect("Unable to get peer list");

        assert_eq!(peer_list, vec!["test_peer".to_string()]);

        peer_manager.shutdown_handle().unwrap().shutdown();
        cm.shutdown_signaler().shutdown();
        peer_manager.await_shutdown();
        cm.await_shutdown();
        mesh.shutdown_signaler().shutdown();
    }

    struct NoopAuthorizer {
        ids: std::cell::RefCell<VecDeque<String>>,
    }

    impl NoopAuthorizer {
        fn new(id: &str) -> Self {
            let mut ids = VecDeque::new();
            ids.push_back(id.into());
            Self {
                ids: std::cell::RefCell::new(ids),
            }
        }

        fn new_multiple(ids: &[&str]) -> Self {
            Self {
                ids: std::cell::RefCell::new(
                    ids.iter().map(std::string::ToString::to_string).collect(),
                ),
            }
        }
    }

    impl Authorizer for NoopAuthorizer {
        fn authorize_connection(
            &self,
            connection_id: String,
            connection: Box<dyn Connection>,
            callback: Box<
                dyn Fn(AuthorizationResult) -> Result<(), Box<dyn std::error::Error>> + Send,
            >,
        ) -> Result<(), AuthorizerError> {
            let identity = self
                .ids
                .borrow_mut()
                .pop_front()
                .expect("No more identities to provide");
            (*callback)(AuthorizationResult::Authorized {
                connection_id,
                connection,
                identity,
            })
            .map_err(|err| AuthorizerError(format!("Unable to return result: {}", err)))
        }
    }
}
