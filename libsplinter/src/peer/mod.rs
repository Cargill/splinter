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
pub use crate::peer::connector::PeerManagerConnector;
use crate::peer::connector::PeerRemover;
pub use crate::peer::notification::{PeerManagerNotification, PeerNotificationIter};
use crate::peer::peer_map::{PeerMap, PeerStatus};
use crate::threading::pacemaker;

use uuid::Uuid;

// the number of retry attempts for an active endpoint before the PeerManager will try other
// endpoints associated with a peer
const DEFAULT_MAXIMUM_RETRY_ATTEMPTS: u64 = 5;
// Default value of how often the Pacemaker should send RetryPending message
const DEFAULT_PACEMAKER_INTERVAL: u64 = 10;
// Default value for maximum time between retrying a peers endpoints
const DEFAULT_MAXIMUM_RETRY_FREQUENCY: u64 = 300;
// Default intial value for how long to wait before retrying a peers endpoints
const INITIAL_RETRY_FREQUENCY: u64 = 10;
// How often to retry connecting to requested peers without id
const REQUESTED_ENDPOINTS_RETRY_FREQUENCY: u64 = 60;

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
        sender: Sender<Result<EndpointPeerRef, PeerUnknownAddError>>,
    },
    RemovePeer {
        peer_id: String,
        sender: Sender<Result<(), PeerRefRemoveError>>,
    },
    RemovePeerByEndpoint {
        endpoint: String,
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

#[derive(Debug, PartialEq)]
pub struct EndpointPeerRef {
    endpoint: String,
    peer_remover: PeerRemover,
}

impl EndpointPeerRef {
    pub(super) fn new(endpoint: String, peer_remover: PeerRemover) -> Self {
        EndpointPeerRef {
            endpoint,
            peer_remover,
        }
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

impl Drop for EndpointPeerRef {
    fn drop(&mut self) {
        match self
            .peer_remover
            .remove_peer_ref_by_endpoint(&self.endpoint)
        {
            Ok(_) => (),
            Err(err) => error!(
                "Unable to remove reference to peer with endpoint {} on drop: {}",
                self.endpoint, err
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

struct UnreferencedPeerState {
    peers: HashMap<String, UnreferencedPeer>,
    // the list of endpoints that have been requested without an id
    requested_endpoints: Vec<String>,
    // last time the unsuccessful requested endpoints have been retried
    last_connection_attempt: Instant,
    // how often to retry to connection to requested endpoints
    retry_frequency: u64,
}

impl UnreferencedPeerState {
    fn new() -> Self {
        UnreferencedPeerState {
            peers: HashMap::default(),
            requested_endpoints: Vec::default(),
            last_connection_attempt: Instant::now(),
            retry_frequency: REQUESTED_ENDPOINTS_RETRY_FREQUENCY,
        }
    }
}

/// The PeerManager is in charge of keeping track of peers and their ref count, as well as
/// requesting connections from the ConnectionManager. If a peer has disconnected, the PeerManager
/// will also try the peer's other endpoints until one is successful.
pub struct PeerManager {
    connection_manager_connector: Connector,
    join_handle: Option<thread::JoinHandle<()>>,
    sender: Option<Sender<PeerManagerMessage>>,
    shutdown_handle: Option<ShutdownHandle>,
    max_retry_attempts: u64,
    retry_interval: u64,
    identity: String,
    strict_ref_counts: bool,
}

impl PeerManager {
    pub fn new(
        connector: Connector,
        max_retry_attempts: Option<u64>,
        retry_interval: Option<u64>,
        identity: String,
        strict_ref_counts: bool,
    ) -> Self {
        PeerManager {
            connection_manager_connector: connector,
            join_handle: None,
            sender: None,
            shutdown_handle: None,
            max_retry_attempts: max_retry_attempts.unwrap_or(DEFAULT_MAXIMUM_RETRY_ATTEMPTS),
            retry_interval: retry_interval.unwrap_or(DEFAULT_PACEMAKER_INTERVAL),
            identity,
            strict_ref_counts,
        }
    }

    /// Start the PeerManager
    ///
    /// Returns a PeerManagerConnector that can be used to send requests to the PeerManager.
    pub fn start(&mut self) -> Result<PeerManagerConnector, PeerManagerError> {
        debug!(
            "Starting peer manager with retry_interval={}s, max_retry_attempts={} and \
            strict_ref_counts={}",
            &self.retry_interval, &self.max_retry_attempts, &self.strict_ref_counts
        );

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

        debug!(
            "Starting peer manager pacemaker with interval of {}s",
            &self.retry_interval
        );
        let pacemaker = pacemaker::Pacemaker::builder()
            .with_interval(self.retry_interval)
            .with_sender(sender.clone())
            .with_message_factory(|| PeerManagerMessage::RetryPending)
            .start()
            .map_err(|err| PeerManagerError::StartUpError(err.to_string()))?;

        let pacemaker_shutdown_signaler = pacemaker.shutdown_signaler();
        let max_retry_attempts = self.max_retry_attempts;

        let identity = self.identity.to_string();
        let strict_ref_counts = self.strict_ref_counts;
        let join_handle = thread::Builder::new()
            .name("Peer Manager".into())
            .spawn(move || {
                let mut peers = PeerMap::new(INITIAL_RETRY_FREQUENCY);
                // a map of identities to unreferenced peers.
                // and a list of endpoints that should be turned into peers
                let mut unreferenced_peers = UnreferencedPeerState::new();
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
                                &mut subscribers,
                                strict_ref_counts,
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
                                &identity,
                                &mut ref_map,
                            )
                        }
                        Ok(PeerManagerMessage::RetryPending) => {
                            retry_pending(&mut peers, connector.clone(), &mut unreferenced_peers)
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

                debug!("Shutting down peer manager pacemaker...");
                pacemaker.await_shutdown();
                debug!("Shutting down peer manager pacemaker (complete)");
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
        debug!("Shutting down peer manager...");
        let join_handle = if let Some(jh) = self.join_handle {
            jh
        } else {
            debug!("Shutting down peer manager (complete, no threads existed)");
            return;
        };

        if let Err(err) = join_handle.join() {
            error!("Peer manager thread did not shutdown correctly: {:?}", err);
        }
        debug!("Shutting down peer manager (complete)");
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

// Allow clippy errors for too_many_arguments. The arguments are required
// to avoid needing a lock in the PeerManager.
#[allow(clippy::too_many_arguments)]
fn handle_request(
    request: PeerManagerRequest,
    connector: Connector,
    unreferenced_peers: &mut UnreferencedPeerState,
    peers: &mut PeerMap,
    peer_remover: &PeerRemover,
    ref_map: &mut RefMap,
    subscribers: &mut Vec<Sender<PeerManagerNotification>>,
    strict_ref_counts: bool,
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
                    subscribers,
                ))
                .is_err()
            {
                warn!("connector dropped before receiving result of adding peer");
            }
        }
        PeerManagerRequest::AddUnidentified { endpoint, sender } => {
            if sender
                .send(add_unidentified(
                    endpoint,
                    connector,
                    unreferenced_peers,
                    peer_remover,
                    peers,
                    ref_map,
                ))
                .is_err()
            {
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
                    strict_ref_counts,
                ))
                .is_err()
            {
                warn!("connector dropped before receiving result of removing peer");
            }
        }
        PeerManagerRequest::RemovePeerByEndpoint { endpoint, sender } => {
            if sender
                .send(remove_peer_by_endpoint(
                    endpoint,
                    connector,
                    peers,
                    ref_map,
                    strict_ref_counts,
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
            let peer_ids = unreferenced_peers
                .peers
                .keys()
                .map(|s| s.to_owned())
                .collect();
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
                        .peers
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
                        .peers
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

// Allow clippy errors for too_many_arguments. The arguments are required
// to avoid needing a lock in the PeerManager.
#[allow(clippy::too_many_arguments)]
fn add_peer(
    peer_id: String,
    endpoints: Vec<String>,
    connector: Connector,
    unreferenced_peers: &mut UnreferencedPeerState,
    peers: &mut PeerMap,
    peer_remover: &PeerRemover,
    ref_map: &mut RefMap,
    subscribers: &mut Vec<Sender<PeerManagerNotification>>,
) -> Result<PeerRef, PeerRefAddError> {
    let new_ref_count = ref_map.add_ref(peer_id.to_string());

    // if this is not a new peer, return success
    if new_ref_count > 1 {
        if let Some(mut peer_metadata) = peers.get_by_peer_id(&peer_id).cloned() {
            if peer_metadata.endpoints.len() == 1 && endpoints.len() > 1 {
                // this should always be true
                if let Some(endpoint) = peer_metadata.endpoints.get(0) {
                    // if peer was add by endpoint, its peer metadata should be updated to include
                    // the full list of endpoints in this request
                    if unreferenced_peers.requested_endpoints.contains(endpoint)
                        && endpoints.contains(&endpoint)
                    {
                        info!(
                            "Updating peer {} to include endpoints {:?}",
                            peer_id, endpoints
                        );
                        peer_metadata.endpoints = endpoints;
                        peers.update_peer(peer_metadata.clone()).map_err(|err| {
                            PeerRefAddError::AddError(format!(
                                "Unable to update peer {}:{}",
                                peer_id, err
                            ))
                        })?
                    } else {
                        // remove ref we just added
                        if let Err(err) = ref_map.remove_ref(&peer_id) {
                            error!(
                                "Unable to remove ref that was just added for peer {}: {}",
                                peer_id, err
                            );
                        };

                        return Err(PeerRefAddError::AddError(format!(
                            "Mismatch betwen existing and requested peer endpoints: {:?} does not \
                            contain {}",
                            endpoints, endpoint
                        )));
                    }
                } else {
                    return Err(PeerRefAddError::AddError(format!(
                        "Peer {} does not have any endpoints",
                        peer_id
                    )));
                }
            }

            // notify subscribers this peer is connected
            if peer_metadata.status == PeerStatus::Connected {
                // Update peer for new state
                let notification = PeerManagerNotification::Connected {
                    peer: peer_id.to_string(),
                };
                subscribers.retain(|sender| sender.send(notification.clone()).is_ok());
            }

            let peer_ref = PeerRef::new(peer_id, peer_remover.clone());
            return Ok(peer_ref);
        } else {
            return Err(PeerRefAddError::AddError(format!(
                "A reference exists for peer {} but missing peer metadata",
                peer_id
            )));
        }
    };

    // if it is a unreferenced peer, promote it to a fully-referenced peer
    if let Some(UnreferencedPeer {
        connection_id,
        endpoint,
    }) = unreferenced_peers.peers.remove(&peer_id)
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
            // remove ref we just added
            if let Err(err) = ref_map.remove_ref(&peer_id) {
                error!(
                    "Unable to remove ref that was just added for peer {}: {}",
                    peer_id, err
                );
            };
            return Err(PeerRefAddError::AddError(format!(
                "No endpoints provided for peer {}",
                peer_id
            )));
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
fn add_unidentified(
    endpoint: String,
    connector: Connector,
    unreferenced_peers: &mut UnreferencedPeerState,
    peer_remover: &PeerRemover,
    peers: &PeerMap,
    ref_map: &mut RefMap,
) -> Result<EndpointPeerRef, PeerUnknownAddError> {
    debug!("Attempting to peer with peer by endpoint {}", endpoint);
    if let Some(peer_metadata) = peers.get_peer_from_endpoint(&endpoint) {
        // if there is peer in the peer_map, there is reference in the ref map
        ref_map.add_ref(peer_metadata.id.to_string());
        Ok(EndpointPeerRef::new(endpoint, peer_remover.clone()))
    } else {
        let connection_id = format!("{}", Uuid::new_v4());
        match connector.request_connection(&endpoint, &connection_id) {
            Ok(()) => (),
            Err(err) => {
                warn!("Unable to peer with peer at {}: {}", endpoint, err);
            }
        };
        unreferenced_peers
            .requested_endpoints
            .push(endpoint.to_string());
        Ok(EndpointPeerRef::new(endpoint, peer_remover.clone()))
    }
}

fn remove_peer(
    peer_id: String,
    connector: Connector,
    unreferenced_peers: &mut UnreferencedPeerState,
    peers: &mut PeerMap,
    ref_map: &mut RefMap,
    strict_ref_counts: bool,
) -> Result<(), PeerRefRemoveError> {
    debug!("Removing peer: {}", peer_id);

    // remove from the unreferenced peers, if it is there.
    unreferenced_peers.peers.remove(&peer_id);

    // remove the reference
    let removed_peer = match ref_map.remove_ref(&peer_id) {
        Ok(removed_peer) => removed_peer,
        Err(err) => {
            if strict_ref_counts {
                panic!(
                    "Trying to remove a reference that does not exist {}",
                    peer_id
                );
            } else {
                return Err(PeerRefRemoveError::RemoveError(format!(
                    "Failed to remove ref for peer {} from ref map: {}",
                    peer_id, err
                )));
            }
        }
    };

    if let Some(removed_peer) = removed_peer {
        let peer_metadata = peers.remove(&removed_peer).ok_or_else(|| {
            PeerRefRemoveError::RemoveError(format!(
                "Peer {} has already been removed from the peer map",
                peer_id
            ))
        })?;

        // If the peer is pending there is no connection to remove
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

fn remove_peer_by_endpoint(
    endpoint: String,
    connector: Connector,
    peers: &mut PeerMap,
    ref_map: &mut RefMap,
    strict_ref_counts: bool,
) -> Result<(), PeerRefRemoveError> {
    let peer_metadata = match peers.get_peer_from_endpoint(&endpoint) {
        Some(peer_metadata) => peer_metadata,
        None => {
            return Err(PeerRefRemoveError::RemoveError(format!(
                "Peer with endpoint {} has already been removed from the peer map",
                endpoint
            )))
        }
    };

    debug!(
        "Removing peer {} by endpoint: {}",
        peer_metadata.id, endpoint
    );
    // remove the reference
    let removed_peer = match ref_map.remove_ref(&peer_metadata.id) {
        Ok(removed_peer) => removed_peer,
        Err(err) => {
            if strict_ref_counts {
                panic!(
                    "Trying to remove a reference that does not exist {}",
                    peer_metadata.id
                );
            } else {
                return Err(PeerRefRemoveError::RemoveError(format!(
                    "Failed to remove ref for peer {} from ref map: {}",
                    peer_metadata.id, err
                )));
            }
        }
    };
    if let Some(removed_peer) = removed_peer {
        let peer_metadata = peers.remove(&removed_peer).ok_or_else(|| {
            PeerRefRemoveError::RemoveError(format!(
                "Peer with endpoint {} has already been removed from the peer map",
                endpoint
            ))
        })?;

        // If the peer is pending there is no connection to remove
        if peer_metadata.status == PeerStatus::Pending {
            return Ok(());
        }

        match connector.remove_connection(&peer_metadata.active_endpoint) {
            Ok(Some(_)) => {
                debug!(
                    "Peer {} has been removed and connection {} has been closed",
                    peer_metadata.id, peer_metadata.active_endpoint
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

// Allow clippy errors for too_many_arguments. The arguments are required
// to avoid needing a lock in the PeerManager.
#[allow(clippy::too_many_arguments)]
fn handle_notifications(
    notification: ConnectionManagerNotification,
    unreferenced_peers: &mut UnreferencedPeerState,
    peers: &mut PeerMap,
    connector: Connector,
    subscribers: &mut Vec<Sender<PeerManagerNotification>>,
    max_retry_attempts: u64,
    local_identity: &str,
    ref_map: &mut RefMap,
) {
    match notification {
        // If a connection has disconnected, forward notification to subscribers
        ConnectionManagerNotification::Disconnected { endpoint, identity } => handle_disconnection(
            endpoint,
            identity,
            unreferenced_peers,
            peers,
            connector,
            subscribers,
        ),
        ConnectionManagerNotification::NonFatalConnectionError {
            endpoint,
            attempts,
            identity,
        } => {
            // Check if the disconnected peer has reached the retry limit, if so try to find a
            // different endpoint that can be connected to
            if let Some(mut peer_metadata) = peers.get_by_peer_id(&identity).cloned() {
                warn!("Received non fatal connection with attempts: {}", attempts);
                if attempts >= max_retry_attempts {
                    if endpoint != peer_metadata.active_endpoint {
                        warn!(
                            "Received non fatal connection notification for peer {} with \
                            different endpoint {}",
                            identity, endpoint
                        );
                        return;
                    };
                    info!("Attempting to find available endpoint for {}", identity);
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
                }

                peer_metadata.status = PeerStatus::Disconnected {
                    retry_attempts: attempts,
                };

                if let Err(err) = peers.update_peer(peer_metadata) {
                    error!("Unable to update peer: {}", err);
                }
            }
        }
        ConnectionManagerNotification::InboundConnection {
            endpoint,
            connection_id,
            identity,
        } => handle_inbound_connection(
            endpoint,
            identity,
            connection_id,
            unreferenced_peers,
            peers,
            connector,
            subscribers,
            local_identity,
        ),
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
            local_identity,
            ref_map,
        ),
        ConnectionManagerNotification::FatalConnectionError { endpoint, error } => {
            handle_fatal_connection(endpoint, error.to_string(), peers, subscribers)
        }
    }
}

fn handle_disconnection(
    endpoint: String,
    identity: String,
    unreferenced_peers: &mut UnreferencedPeerState,
    peers: &mut PeerMap,
    connector: Connector,
    subscribers: &mut Vec<Sender<PeerManagerNotification>>,
) {
    if let Some(mut peer_metadata) = peers.get_by_peer_id(&identity).cloned() {
        if endpoint != peer_metadata.active_endpoint {
            warn!(
                "Received disconnection notification for peer {} with \
                different endpoint {}",
                identity, endpoint
            );
            return;
        }

        let notification = PeerManagerNotification::Disconnected {
            peer: peer_metadata.id.to_string(),
        };
        info!("Peer {} is currently disconnected", identity);
        if peer_metadata.endpoints.contains(&endpoint) {
            // allow peer manager to retry connection to that endpoint until the retry max is
            // reached

            // set peer to disconnected
            peer_metadata.status = PeerStatus::Disconnected { retry_attempts: 1 };
            if let Err(err) = peers.update_peer(peer_metadata) {
                error!("Unable to update peer: {}", err);
            }
        } else {
            // the disconnected endpoint is an inbound connection. This connection should
            // be removed, peer set to pending and the endpoints in the peer metadata
            // should be tried
            if let Err(err) = connector.remove_connection(&peer_metadata.active_endpoint) {
                error!("Unable to clean up old connection: {}", err);
            }

            info!("Attempting to find available endpoint for {}", identity);
            for endpoint in peer_metadata.endpoints.iter() {
                match connector.request_connection(&endpoint, &peer_metadata.connection_id) {
                    Ok(()) => break,
                    Err(err) => error!(
                        "Unable to request connection for peer {} at endpoint {}: {}",
                        peer_metadata.id, endpoint, err
                    ),
                }
            }
            peer_metadata.status = PeerStatus::Pending;
            if let Err(err) = peers.update_peer(peer_metadata) {
                error!("Unable to update peer: {}", err);
            }
        }
        subscribers.retain(|sender| sender.send(notification.clone()).is_ok());
    } else {
        // check for unreferenced peer and remove if it has disconnected
        debug!("Removing disconnected peer: {}", identity);
        if let Some(unref_peer) = unreferenced_peers.peers.remove(&identity) {
            if let Err(err) = connector.remove_connection(&unref_peer.endpoint) {
                error!("Unable to clean up old connection: {}", err);
            }
        }
    }
}

// Allow clippy errors for too_many_arguments. The arguments are required
// to avoid needing a lock in the PeerManager.
#[allow(clippy::too_many_arguments)]
fn handle_inbound_connection(
    endpoint: String,
    identity: String,
    connection_id: String,
    unreferenced_peers: &mut UnreferencedPeerState,
    peers: &mut PeerMap,
    connector: Connector,
    subscribers: &mut Vec<Sender<PeerManagerNotification>>,
    local_identity: &str,
) {
    info!(
        "Received peer connection from {} (remote endpoint: {})",
        identity, endpoint
    );

    // If we got an inbound counnection for an existing peer, replace old connection with
    // this new one unless we are already connected.
    if let Some(mut peer_metadata) = peers.get_by_peer_id(&identity).cloned() {
        match peer_metadata.status {
            PeerStatus::Disconnected { .. } => {
                info!(
                    "Adding inbound connection to Disconnected peer: {}",
                    identity
                );
            }
            PeerStatus::Pending => {
                info!("Adding inbound connection to Pending peer: {}", identity);
            }
            PeerStatus::Connected => {
                // Compare identities, if local identity is greater, close incoming connection
                // otherwise, remove outbound connection and replace with inbound.
                if local_identity > identity.as_str() {
                    // if peer is already connected, remove the inbound connection
                    info!(
                        "Removing inbound connection, already connected to {}",
                        peer_metadata.id
                    );
                    if let Err(err) = connector.remove_connection(&endpoint) {
                        error!("Unable to clean up old connection: {}", err);
                    }
                    return;
                } else {
                    info!(
                        "Replacing existing connection with inbound for peer {}",
                        peer_metadata.id
                    );
                }
            }
        }
        let old_endpoint = peer_metadata.active_endpoint;
        let starting_status = peer_metadata.status;
        peer_metadata.status = PeerStatus::Connected;
        peer_metadata.connection_id = connection_id;
        // reset retry settings
        peer_metadata.retry_frequency = INITIAL_RETRY_FREQUENCY;
        peer_metadata.last_connection_attempt = Instant::now();

        let notification = PeerManagerNotification::Connected {
            peer: peer_metadata.id.to_string(),
        };

        peer_metadata.active_endpoint = endpoint.to_string();
        if let Err(err) = peers.update_peer(peer_metadata) {
            error!("Unable to update peer: {}", err);
        }

        subscribers.retain(|sender| sender.send(notification.clone()).is_ok());

        // if peer is pending there is no connection to remove
        if endpoint != old_endpoint && starting_status != PeerStatus::Pending {
            if let Err(err) = connector.remove_connection(&old_endpoint) {
                warn!("Unable to clean up old connection: {}", err);
            }
        }
    } else {
        debug!("Adding peer with id: {}", identity);

        if let Some(old_peer) = unreferenced_peers.peers.insert(
            identity,
            UnreferencedPeer {
                connection_id,
                endpoint: endpoint.to_string(),
            },
        ) {
            if old_peer.endpoint != endpoint {
                debug!("Removing old peer connection for {}", old_peer.endpoint);
                if let Err(err) = connector.remove_connection(&old_peer.endpoint) {
                    error!("Unable to clean up old connection: {}", err);
                }
            }
        }
    }
}

// Allow clippy errors for too_many_arguments and cognitive_complexity. The arguments are required
// to avoid needing a lock in the PeerManager.
#[allow(clippy::too_many_arguments, clippy::cognitive_complexity)]
fn handle_connected(
    endpoint: String,
    identity: String,
    connection_id: String,
    unreferenced_peers: &mut UnreferencedPeerState,
    peers: &mut PeerMap,
    connector: Connector,
    subscribers: &mut Vec<Sender<PeerManagerNotification>>,
    local_identity: &str,
    ref_map: &mut RefMap,
) {
    if let Some(mut peer_metadata) = peers.get_peer_from_endpoint(&endpoint).cloned() {
        match peer_metadata.status {
            PeerStatus::Pending => {
                info!(
                    "Pending peer {} connected via {}",
                    peer_metadata.id, endpoint
                );
            }
            PeerStatus::Disconnected { .. } => {
                info!(
                    "Disconnected peer {} connected via {}",
                    peer_metadata.id, endpoint
                );
            }
            PeerStatus::Connected => {
                // Compare identities, if remote identity is greater, remove outbound connection
                // otherwise replace inbound connection with outbound.
                if local_identity < identity.as_str() {
                    info!(
                        "Removing outbound connection, peer {} is already connected",
                        peer_metadata.id
                    );
                    // we are already connected on another connection, remove this connection
                    if endpoint != peer_metadata.active_endpoint {
                        if let Err(err) = connector.remove_connection(&endpoint) {
                            error!("Unable to clean up old connection: {}", err);
                        }
                    }
                    return;
                } else {
                    info!(
                        "Connected Peer {} connected via {}",
                        peer_metadata.id, endpoint
                    );
                }
            }
        }

        if identity != peer_metadata.id {
            // remove connection that has provided mismatched identity
            if let Err(err) = connector.remove_connection(&endpoint) {
                error!("Unable to clean up mismatched identity connection: {}", err);
            }

            // also remove current active endpoint because peer is currently invalid
            if let Err(err) = connector.remove_connection(&peer_metadata.active_endpoint) {
                error!("Unable to clean up mismatched identity connection: {}", err);
            }

            // tell subscribers this Peer is currently disconnected
            let notification = PeerManagerNotification::Disconnected {
                peer: peer_metadata.id.to_string(),
            };

            // set its status to pending, this will cause the endpoints to be retried at
            // a later time
            peer_metadata.status = PeerStatus::Pending;

            error!(
                "Peer {} (via {}) presented a mismatched identity {}",
                identity, endpoint, peer_metadata.id
            );

            if let Err(err) = peers.update_peer(peer_metadata) {
                error!("Unable to update peer: {}", err);
            }

            subscribers.retain(|sender| sender.send(notification.clone()).is_ok());
            return;
        }

        // Update peer for new state
        let notification = PeerManagerNotification::Connected {
            peer: peer_metadata.id.to_string(),
        };

        let starting_status = peer_metadata.status;
        let old_endpoint = peer_metadata.active_endpoint;
        peer_metadata.active_endpoint = endpoint.to_string();
        peer_metadata.status = PeerStatus::Connected;
        peer_metadata.connection_id = connection_id;
        // reset retry settings
        peer_metadata.retry_frequency = INITIAL_RETRY_FREQUENCY;
        peer_metadata.last_connection_attempt = Instant::now();

        if let Err(err) = peers.update_peer(peer_metadata) {
            error!("Unable to update peer: {}", err);
        }

        // remove old connection
        if endpoint != old_endpoint && starting_status != PeerStatus::Pending {
            if let Err(err) = connector.remove_connection(&old_endpoint) {
                error!("Unable to clean up old connection: {}", err);
            }
        }

        // notify subscribers we are connected
        subscribers.retain(|sender| sender.send(notification.clone()).is_ok());
    } else {
        debug!("Adding peer {} by endpoint {}", identity, endpoint);

        // if this endpoint has been requested, add this connection to peers with the provided
        // endpoint
        if unreferenced_peers.requested_endpoints.contains(&endpoint) {
            ref_map.add_ref(identity.to_string());
            peers.insert(
                identity.to_string(),
                connection_id,
                vec![endpoint.to_string()],
                endpoint,
                PeerStatus::Connected,
            );

            let notification = PeerManagerNotification::Connected { peer: identity };
            subscribers.retain(|sender| sender.send(notification.clone()).is_ok());
            return;
        }

        // Treat unknown peer as unreferenced
        if let Some(old_peer) = unreferenced_peers.peers.insert(
            identity,
            UnreferencedPeer {
                connection_id,
                endpoint: endpoint.to_string(),
            },
        ) {
            if old_peer.endpoint != endpoint {
                debug!("Removing old peer connection for {}", old_peer.endpoint);
                if let Err(err) = connector.remove_connection(&old_peer.endpoint) {
                    error!("Unable to clean up old connection: {}", err);
                }
            }
        }
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
            "Peer {} encountered a fatal connection error: {}",
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
fn retry_pending(
    peers: &mut PeerMap,
    connector: Connector,
    unreferenced_peers: &mut UnreferencedPeerState,
) {
    let mut to_retry = Vec::new();
    for (_, peer) in peers.get_pending() {
        if peer.last_connection_attempt.elapsed().as_secs() > peer.retry_frequency {
            to_retry.push(peer.clone());
        }
    }

    for mut peer_metadata in to_retry {
        debug!("Retry peering with pending peer {}", peer_metadata.id);
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

    if unreferenced_peers
        .last_connection_attempt
        .elapsed()
        .as_secs()
        > unreferenced_peers.retry_frequency
    {
        for endpoint in unreferenced_peers.requested_endpoints.iter() {
            if peers.contains_endpoint(endpoint) {
                continue;
            }
            debug!("Attempting to peer with peer by {}", endpoint);
            let connection_id = format!("{}", Uuid::new_v4());
            match connector.request_connection(&endpoint, &connection_id) {
                Ok(()) => (),
                // If request_connection errored we will retry in the future
                Err(err) => {
                    error!(
                        "Unable to request connection for peer endpoint {}: {}",
                        endpoint, err
                    );
                }
            }
        }

        unreferenced_peers.last_connection_attempt = Instant::now();
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
        let mut peer_manager =
            PeerManager::new(connector, None, Some(1), "my_id".to_string(), true);
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
    // 4. validate a Disconnected notification is returned,
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
        let mut peer_manager =
            PeerManager::new(connector.clone(), None, Some(1), "my_id".to_string(), true);

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
        let mut peer_manager =
            PeerManager::new(connector.clone(), None, Some(1), "my_id".to_string(), true);

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
        let mut peer_manager =
            PeerManager::new(connector, None, Some(1), "my_id".to_string(), true);
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
        let mut peer_manager =
            PeerManager::new(connector, None, Some(1), "my_id".to_string(), true);
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
        let mut peer_manager =
            PeerManager::new(connector, None, Some(1), "my_id".to_string(), true);
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
        let mut peer_manager =
            PeerManager::new(connector, None, Some(1), "my_id".to_string(), true);

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

    // Test that when a EndpointPeerRef is dropped, a remove peer request is properly sent and the
    // peer is removed
    //
    //
    // 1. add unidentified peer with endpoint inproc://test
    // 2. add test_peer
    // 4. call list peers
    // 5. verify that the peer list contains test_peer
    // 6. drop the PeerRef
    // 7. call list peers
    // 8. verify that the peer list still contains test_peer
    // 9. drop endpoint peer_ref
    // 10. call list peers
    // 11. verify that the new peer list is empty
    #[test]
    fn test_peer_manager_drop_endpoint_peer_ref() {
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

        // let (finish_tx, fininsh_rx) = channel();
        let connector = cm.connector();
        let mut peer_manager =
            PeerManager::new(connector, None, Some(1), "my_id".to_string(), true);

        let peer_connector = peer_manager.start().expect("Cannot start peer_manager");

        {
            let mut subscriber = peer_connector
                .subscribe()
                .expect("Unable to get subscriber");
            let endpoint_peer_ref = peer_connector
                .add_unidentified_peer("inproc://test".to_string())
                .expect("Unable to add peer by endpoint");
            assert_eq!(endpoint_peer_ref.endpoint(), "inproc://test".to_string());
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

            let peer_list = peer_connector
                .list_peers()
                .expect("Unable to get peer list");

            assert_eq!(peer_list, vec!["test_peer".to_string()]);

            drop(peer_ref);

            let peer_list = peer_connector
                .list_peers()
                .expect("Unable to get peer list");

            assert_eq!(peer_list, vec!["test_peer".to_string()]);
        }
        // drop endpoint_peer_ref

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
    // 6. verify that subscribers will receive a Connected notification when the new active endpoint
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
            .with_heartbeat_interval(1)
            .start()
            .expect("Unable to start Connection Manager");

        let connector = cm.connector();
        let mut peer_manager =
            PeerManager::new(connector, Some(1), Some(1), "my_id".to_string(), true);
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
        let mut peer_manager =
            PeerManager::new(connector, Some(1), Some(1), "my_id".to_string(), true);
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
            // wait for inbound connection notification to come
            subs_rx.recv().expect("unable to get notification");
        });

        let mut peer_manager =
            PeerManager::new(connector, Some(1), Some(1), "my_id".to_string(), true);
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
