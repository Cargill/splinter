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

pub mod authorizers;
mod error;
mod notification;
mod pacemaker;

use std::cmp::min;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::time::Instant;

use uuid::Uuid;

pub use error::{AuthorizerError, ConnectionManagerError};
pub use notification::{ConnectionManagerNotification, NotificationIter};
use pacemaker::Pacemaker;
use protobuf::Message;

use crate::protos::network::{NetworkHeartbeat, NetworkMessage, NetworkMessageType};
use crate::transport::matrix::{ConnectionMatrixLifeCycle, ConnectionMatrixSender};
use crate::transport::{Connection, Transport};

const DEFAULT_HEARTBEAT_INTERVAL: u64 = 10;
const INITIAL_RETRY_FREQUENCY: u64 = 10;
const DEFAULT_MAXIMUM_RETRY_FREQUENCY: u64 = 300;

pub type AuthorizerCallback =
    Box<dyn Fn(AuthorizationResult) -> Result<(), Box<dyn std::error::Error>> + Send>;

pub trait Authorizer {
    fn authorize_connection(
        &self,
        connection_id: String,
        connection: Box<dyn Connection>,
        on_complete: AuthorizerCallback,
    ) -> Result<(), AuthorizerError>;
}

pub enum AuthorizationResult {
    Authorized {
        connection_id: String,
        identity: String,
        connection: Box<dyn Connection>,
    },
    Unauthorized {
        connection_id: String,
        connection: Box<dyn Connection>,
    },
}

pub type SubscriberId = usize;
type Subscriber =
    Box<dyn Fn(ConnectionManagerNotification) -> Result<(), Box<dyn std::error::Error>> + Send>;

struct SubscriberMap {
    subscribers: HashMap<SubscriberId, Subscriber>,
    next_id: SubscriberId,
}

impl SubscriberMap {
    fn new() -> Self {
        Self {
            subscribers: HashMap::new(),
            next_id: 0,
        }
    }

    fn broadcast(&mut self, notification: ConnectionManagerNotification) {
        let mut failures = vec![];
        for (id, callback) in self.subscribers.iter() {
            if let Err(err) = (*callback)(notification.clone()) {
                failures.push(*id);
                debug!("Dropping subscriber ({}): {}", id, err);
            }
        }

        for id in failures {
            self.subscribers.remove(&id);
        }
    }

    fn add_subscriber(&mut self, subscriber: Subscriber) -> SubscriberId {
        let subscriber_id = self.next_id;
        self.next_id += 1;
        self.subscribers.insert(subscriber_id, subscriber);

        subscriber_id
    }

    fn remove_subscriber(&mut self, subscriber_id: SubscriberId) {
        self.subscribers.remove(&subscriber_id);
    }
}

enum CmMessage {
    Shutdown,
    Request(CmRequest),
    AuthResult(AuthResult),
    SendHeartbeats,
}

enum CmRequest {
    RequestOutboundConnection {
        endpoint: String,
        connection_id: String,
        sender: Sender<Result<String, ConnectionManagerError>>,
    },
    RemoveConnection {
        endpoint: String,
        sender: Sender<Result<Option<String>, ConnectionManagerError>>,
    },
    ListConnections {
        sender: Sender<Result<Vec<String>, ConnectionManagerError>>,
    },
    AddInboundConnection {
        connection: Box<dyn Connection>,
        sender: Sender<Result<(), ConnectionManagerError>>,
    },
    Subscribe {
        sender: Sender<Result<SubscriberId, ConnectionManagerError>>,
        callback: Subscriber,
    },
    Unsubscribe {
        subscriber_id: SubscriberId,
        sender: Sender<Result<(), ConnectionManagerError>>,
    },
}

enum AuthResult {
    Outbound {
        endpoint: String,
        sender: Sender<Result<String, ConnectionManagerError>>,
        auth_result: AuthorizationResult,
    },
    Inbound {
        endpoint: String,
        sender: Sender<Result<(), ConnectionManagerError>>,
        auth_result: AuthorizationResult,
    },
}

pub struct ConnectionManager<T: 'static, U: 'static>
where
    T: ConnectionMatrixLifeCycle,
    U: ConnectionMatrixSender,
{
    pacemaker: Pacemaker,
    connection_state: Option<ConnectionManagerState<T, U>>,
    authorizer: Option<Box<dyn Authorizer + Send>>,
    join_handle: Option<thread::JoinHandle<()>>,
    sender: Option<Sender<CmMessage>>,
    shutdown_signaler: Option<ShutdownSignaler>,
}

impl<T, U> ConnectionManager<T, U>
where
    T: ConnectionMatrixLifeCycle,
    U: ConnectionMatrixSender,
{
    pub fn new(
        authorizer: Box<dyn Authorizer + Send>,
        life_cycle: T,
        matrix_sender: U,
        transport: Box<dyn Transport + Send>,
        heartbeat_interval: Option<u64>,
        maximum_retry_frequency: Option<u64>,
    ) -> Self {
        let heartbeat = heartbeat_interval.unwrap_or(DEFAULT_HEARTBEAT_INTERVAL);
        let retry_frequency = maximum_retry_frequency.unwrap_or(DEFAULT_MAXIMUM_RETRY_FREQUENCY);
        let connection_state = Some(ConnectionManagerState::new(
            life_cycle,
            matrix_sender,
            transport,
            retry_frequency,
        ));
        let pacemaker = Pacemaker::new(heartbeat);

        Self {
            authorizer: Some(authorizer),
            pacemaker,
            connection_state,
            join_handle: None,
            sender: None,
            shutdown_signaler: None,
        }
    }

    pub fn start(&mut self) -> Result<Connector, ConnectionManagerError> {
        let (sender, recv) = channel();
        let mut state = self.connection_state.take().ok_or_else(|| {
            ConnectionManagerError::StartUpError("Service has already started".into())
        })?;

        let authorizer = self.authorizer.take().ok_or_else(|| {
            ConnectionManagerError::StartUpError("Service has already started".into())
        })?;

        let resender = sender.clone();
        let join_handle = thread::Builder::new()
            .name("Connection Manager".into())
            .spawn(move || {
                let mut subscribers = SubscriberMap::new();
                loop {
                    match recv.recv() {
                        Ok(CmMessage::Shutdown) => break,
                        Ok(CmMessage::Request(req)) => {
                            handle_request(
                                req,
                                &mut state,
                                &mut subscribers,
                                &*authorizer,
                                resender.clone(),
                            );
                        }
                        Ok(CmMessage::AuthResult(auth_result)) => {
                            handle_auth_result(auth_result, &mut state, &mut subscribers);
                        }
                        Ok(CmMessage::SendHeartbeats) => {
                            send_heartbeats(&mut state, &mut subscribers)
                        }
                        Err(_) => {
                            warn!("All senders have disconnected");
                            break;
                        }
                    }
                }
            })?;

        self.pacemaker
            .start(sender.clone(), || CmMessage::SendHeartbeats)?;
        self.join_handle = Some(join_handle);
        self.shutdown_signaler = Some(ShutdownSignaler {
            sender: sender.clone(),
            pacemaker_shutdown_signaler: self.pacemaker.shutdown_signaler().unwrap(),
        });
        self.sender = Some(sender.clone());

        Ok(Connector { sender })
    }

    pub fn shutdown_signaler(&self) -> Option<ShutdownSignaler> {
        self.shutdown_signaler.clone()
    }

    pub fn await_shutdown(self) {
        self.pacemaker.await_shutdown();

        let join_handle = if let Some(jh) = self.join_handle {
            jh
        } else {
            return;
        };

        if let Err(err) = join_handle.join() {
            error!(
                "Connection manager thread did not shutdown correctly: {:?}",
                err
            );
        }
    }
}

#[derive(Clone)]
pub struct Connector {
    sender: Sender<CmMessage>,
}

impl Connector {
    /// Request a connection to the given endpoint.
    ///
    /// This operation is idempotent: if a connection to that endpoint already exists, a new
    /// connection is not created. On successful connection, the authorized identity of the
    /// connection is returned.
    ///
    /// # Errors
    ///
    /// An error is returned if the connection cannot be created
    pub fn request_connection(
        &self,
        endpoint: &str,
        connection_id: &str,
    ) -> Result<String, ConnectionManagerError> {
        let (sender, recv) = channel();
        self.sender
            .send(CmMessage::Request(CmRequest::RequestOutboundConnection {
                sender,
                endpoint: endpoint.to_string(),
                connection_id: connection_id.into(),
            }))
            .map_err(|_| {
                ConnectionManagerError::SendMessageError(
                    "The connection manager is no longer running: unable to send request".into(),
                )
            })?;

        recv.recv().map_err(|_| {
            ConnectionManagerError::SendMessageError(
                "The connection manager is no longer running: could not receive response".into(),
            )
        })?
    }

    /// Removes a connection
    ///
    ///  # Returns
    ///
    ///  The endpoint, if the connection exists; None, otherwise.
    ///
    ///  # Errors
    ///
    ///  Returns a ConnectionManagerError if the query cannot be performed.
    pub fn remove_connection(
        &self,
        endpoint: &str,
    ) -> Result<Option<String>, ConnectionManagerError> {
        let (sender, recv) = channel();
        self.sender
            .send(CmMessage::Request(CmRequest::RemoveConnection {
                sender,
                endpoint: endpoint.to_string(),
            }))
            .map_err(|_| {
                ConnectionManagerError::SendMessageError(
                    "The connection manager is no longer running".into(),
                )
            })?;

        recv.recv().map_err(|_| {
            ConnectionManagerError::SendMessageError(
                "The connection manager is no longer running".into(),
            )
        })?
    }

    /// Create an iterator over ConnectionManagerNotification events.
    ///
    /// # Errors
    ///
    /// Return a ConnectionManagerError if the notification iterator cannot be created.
    pub fn subscription_iter(&self) -> Result<NotificationIter, ConnectionManagerError> {
        let (send, recv) = channel();

        self.subscribe(send)?;

        Ok(NotificationIter { recv })
    }

    /// Subscribe to notifications for connection events.
    ///
    /// ConnectionManagerNotification instances will be transformed via type `T`'s implementation
    /// of `From<ConnectionManagerNotification>` and passed to the given sender.
    ///
    /// # Returns
    ///
    /// The subscriber id that can be used for unsubscribing the given sender.
    ///
    /// # Errors
    ///
    /// Return a ConnectionManagerError if the subscriber cannot be registered via the Connector
    /// instance.
    pub fn subscribe<T>(
        &self,
        subscriber: Sender<T>,
    ) -> Result<SubscriberId, ConnectionManagerError>
    where
        T: From<ConnectionManagerNotification> + Send + 'static,
    {
        let (sender, recv) = channel();
        self.sender
            .send(CmMessage::Request(CmRequest::Subscribe {
                sender,
                callback: Box::new(move |notification| {
                    subscriber.send(T::from(notification)).map_err(Box::from)
                }),
            }))
            .map_err(|_| {
                ConnectionManagerError::SendMessageError(
                    "The connection manager is no longer running".into(),
                )
            })?;

        recv.recv().map_err(|_| {
            ConnectionManagerError::SendMessageError(
                "The connection manager is no longer running".into(),
            )
        })?
    }

    pub fn unsubscribe(&self, subscriber_id: SubscriberId) -> Result<(), ConnectionManagerError> {
        let (sender, recv) = channel();
        self.sender
            .send(CmMessage::Request(CmRequest::Unsubscribe {
                subscriber_id,
                sender,
            }))
            .map_err(|_| {
                ConnectionManagerError::SendMessageError(
                    "The connection manager is no longer running".into(),
                )
            })?;

        recv.recv().map_err(|_| {
            ConnectionManagerError::SendMessageError(
                "The connection manager is no longer running".into(),
            )
        })?
    }

    /// List the connections available to this Connector instance.
    ///
    /// # Returns
    ///
    /// Returns a vector of connection endpoints.
    ///
    /// # Errors
    ///
    /// Returns a ConnectionManagerError if the connections cannot be queried.
    pub fn list_connections(&self) -> Result<Vec<String>, ConnectionManagerError> {
        let (sender, recv) = channel();
        self.sender
            .send(CmMessage::Request(CmRequest::ListConnections { sender }))
            .map_err(|_| {
                ConnectionManagerError::SendMessageError(
                    "The connection manager is no longer running".into(),
                )
            })?;

        recv.recv().map_err(|_| {
            ConnectionManagerError::SendMessageError(
                "The connection manager is no longer running".into(),
            )
        })?
    }

    pub fn add_inbound_connection(
        &self,
        connection: Box<dyn Connection>,
    ) -> Result<(), ConnectionManagerError> {
        let (sender, recv) = channel();
        self.sender
            .send(CmMessage::Request(CmRequest::AddInboundConnection {
                connection,
                sender,
            }))
            .map_err(|_| {
                ConnectionManagerError::SendMessageError(
                    "The connection manager is no longer running".into(),
                )
            })?;

        recv.recv().map_err(|_| {
            ConnectionManagerError::SendMessageError(
                "The connection manager is no longer running".into(),
            )
        })?
    }
}

/// Signals shutdown to the ConnectionManager
#[derive(Clone)]
pub struct ShutdownSignaler {
    sender: Sender<CmMessage>,
    pacemaker_shutdown_signaler: pacemaker::ShutdownSignaler,
}

impl ShutdownSignaler {
    /// Signal the ConnectionManager to shutdown.
    pub fn shutdown(self) {
        self.pacemaker_shutdown_signaler.shutdown();

        if self.sender.send(CmMessage::Shutdown).is_err() {
            warn!("Connection manager is no longer running");
        }
    }
}

#[derive(Clone, Debug)]
struct ConnectionMetadata {
    connection_id: String,
    endpoint: String,
    identity: String,
    extended_metadata: ConnectionMetadataExt,
}

impl ConnectionMetadata {
    fn is_outbound(&self) -> bool {
        matches!(self.extended_metadata, ConnectionMetadataExt::Outbound { .. })
    }

    fn connection_id(&self) -> &str {
        &self.connection_id
    }

    fn endpoint(&self) -> &str {
        &self.endpoint
    }

    fn identity(&self) -> &str {
        &self.identity
    }
}

#[derive(Clone, Debug)]
enum ConnectionMetadataExt {
    Outbound {
        reconnecting: bool,
        retry_frequency: u64,
        last_connection_attempt: Instant,
        reconnection_attempts: u64,
    },
    Inbound {
        disconnected: bool,
    },
}

struct ConnectionManagerState<T, U>
where
    T: ConnectionMatrixLifeCycle,
    U: ConnectionMatrixSender,
{
    connections: HashMap<String, ConnectionMetadata>,
    life_cycle: T,
    matrix_sender: U,
    transport: Box<dyn Transport>,
    maximum_retry_frequency: u64,
}

impl<T, U> ConnectionManagerState<T, U>
where
    T: ConnectionMatrixLifeCycle,
    U: ConnectionMatrixSender,
{
    fn new(
        life_cycle: T,
        matrix_sender: U,
        transport: Box<dyn Transport + Send>,
        maximum_retry_frequency: u64,
    ) -> Self {
        Self {
            life_cycle,
            matrix_sender,
            transport,
            connections: HashMap::new(),
            maximum_retry_frequency,
        }
    }

    fn add_inbound_connection(
        &mut self,
        connection: Box<dyn Connection>,
        reply_sender: Sender<Result<(), ConnectionManagerError>>,
        internal_sender: Sender<CmMessage>,
        authorizer: &dyn Authorizer,
    ) {
        let endpoint = connection.remote_endpoint();
        let id = Uuid::new_v4().to_string();

        // add the connection to the authorization pool
        let auth_endpoint = endpoint;
        let auth_sender = reply_sender.clone();
        if let Err(err) = authorizer.authorize_connection(
            id,
            connection,
            Box::new(move |auth_result| {
                internal_sender
                    .send(CmMessage::AuthResult(AuthResult::Inbound {
                        endpoint: auth_endpoint.clone(),
                        sender: auth_sender.clone(),
                        auth_result,
                    }))
                    .map_err(Box::from)
            }),
        ) {
            if reply_sender
                .send(Err(ConnectionManagerError::ConnectionCreationError(
                    err.to_string(),
                )))
                .is_err()
            {
                warn!("connector dropped before receiving result of add connection");
            }
        }
    }

    fn add_outbound_connection(
        &mut self,
        endpoint: &str,
        connection_id: String,
        reply_sender: Sender<Result<String, ConnectionManagerError>>,
        internal_sender: Sender<CmMessage>,
        authorizer: &dyn Authorizer,
    ) {
        if let Some(connection) = self.connections.get(endpoint) {
            let identity = connection.identity().to_string();
            if reply_sender.send(Ok(identity)).is_err() {
                warn!("connector dropped before receiving result of add connection");
            }
        } else {
            match self.transport.connect(endpoint) {
                Ok(connection) => {
                    // add the connection to the authorization pool
                    let auth_endpoint = endpoint.to_string();
                    let auth_sender = reply_sender.clone();
                    if let Err(err) = authorizer.authorize_connection(
                        connection_id,
                        connection,
                        Box::new(move |auth_result| {
                            internal_sender
                                .send(CmMessage::AuthResult(AuthResult::Outbound {
                                    endpoint: auth_endpoint.clone(),
                                    sender: auth_sender.clone(),
                                    auth_result,
                                }))
                                .map_err(Box::from)
                        }),
                    ) {
                        if reply_sender
                            .send(Err(ConnectionManagerError::ConnectionCreationError(
                                err.to_string(),
                            )))
                            .is_err()
                        {
                            warn!("connector dropped before receiving result of add connection");
                        }
                    }
                }
                Err(err) => {
                    if reply_sender
                        .send(Err(ConnectionManagerError::ConnectionCreationError(
                            err.to_string(),
                        )))
                        .is_err()
                    {
                        warn!("connector dropped before receiving result of add connection");
                    }
                }
            }
        }
    }

    fn on_outbound_authorization_complete(
        &mut self,
        endpoint: String,
        auth_result: AuthorizationResult,
    ) -> Result<String, ConnectionManagerError> {
        match auth_result {
            AuthorizationResult::Authorized {
                connection_id,
                connection,
                identity,
            } => {
                self.life_cycle
                    .add(connection, connection_id.clone())
                    .map_err(|err| {
                        ConnectionManagerError::ConnectionCreationError(format!("{:?}", err))
                    })?;

                self.connections.insert(
                    endpoint.clone(),
                    ConnectionMetadata {
                        connection_id,
                        identity: identity.clone(),
                        endpoint,
                        extended_metadata: ConnectionMetadataExt::Outbound {
                            reconnecting: false,
                            retry_frequency: INITIAL_RETRY_FREQUENCY,
                            last_connection_attempt: Instant::now(),
                            reconnection_attempts: 0,
                        },
                    },
                );

                Ok(identity)
            }
            AuthorizationResult::Unauthorized { connection_id, .. } => {
                Err(ConnectionManagerError::Unauthorized(connection_id))
            }
        }
    }

    fn on_inbound_authorization_complete(
        &mut self,
        endpoint: String,
        auth_result: AuthorizationResult,
        subscribers: &mut SubscriberMap,
    ) -> Result<(), ConnectionManagerError> {
        match auth_result {
            AuthorizationResult::Authorized {
                connection_id,
                connection,
                identity,
            } => {
                self.life_cycle
                    .add(connection, connection_id.clone())
                    .map_err(|err| {
                        ConnectionManagerError::ConnectionCreationError(format!("{:?}", err))
                    })?;

                self.connections.insert(
                    endpoint.clone(),
                    ConnectionMetadata {
                        connection_id: connection_id.clone(),
                        endpoint: endpoint.clone(),
                        identity: identity.clone(),
                        extended_metadata: ConnectionMetadataExt::Inbound {
                            disconnected: false,
                        },
                    },
                );

                subscribers.broadcast(ConnectionManagerNotification::InboundConnection {
                    endpoint,
                    connection_id,
                    identity,
                });

                Ok(())
            }
            AuthorizationResult::Unauthorized { connection_id, .. } => {
                Err(ConnectionManagerError::Unauthorized(connection_id))
            }
        }
    }

    fn remove_connection(
        &mut self,
        endpoint: &str,
    ) -> Result<Option<ConnectionMetadata>, ConnectionManagerError> {
        let meta = if let Some(meta) = self.connections.get_mut(endpoint) {
            meta.clone()
        } else {
            return Ok(None);
        };

        self.connections.remove(endpoint);
        // remove mesh id, this may happen before reconnection is attempted
        self.life_cycle
            .remove(meta.connection_id())
            .map_err(|err| {
                ConnectionManagerError::ConnectionRemovalError(format!(
                    "Cannot remove connection {} from life cycle: {}",
                    endpoint, err
                ))
            })?;

        Ok(Some(meta))
    }

    fn reconnect(
        &mut self,
        endpoint: &str,
        subscribers: &mut SubscriberMap,
    ) -> Result<(), ConnectionManagerError> {
        let mut meta = if let Some(meta) = self.connections.get_mut(endpoint) {
            meta.clone()
        } else {
            return Err(ConnectionManagerError::ConnectionRemovalError(
                "Cannot reconnect to endpoint without metadata".into(),
            ));
        };

        if !meta.is_outbound() {
            // Do not attempt to reconnect inbound connections.
            return Ok(());
        }

        if let Ok(connection) = self.transport.connect(endpoint) {
            // remove old mesh id, this may happen before reconnection is attempted
            self.life_cycle
                .remove(meta.connection_id())
                .map_err(|err| {
                    ConnectionManagerError::ConnectionRemovalError(format!(
                        "Cannot remove connection {} from life cycle: {}",
                        endpoint, err
                    ))
                })?;

            // add new connection to mesh
            self.life_cycle
                .add(connection, meta.connection_id().to_string())
                .map_err(|err| {
                    ConnectionManagerError::ConnectionReconnectError(format!("{:?}", err))
                })?;

            // replace mesh id and reset reconnecting fields
            match meta.extended_metadata {
                ConnectionMetadataExt::Outbound {
                    ref mut reconnecting,
                    ref mut retry_frequency,
                    ref mut last_connection_attempt,
                    ref mut reconnection_attempts,
                } => {
                    *reconnecting = false;
                    *retry_frequency = INITIAL_RETRY_FREQUENCY;
                    *last_connection_attempt = Instant::now();
                    *reconnection_attempts = 0;
                }
                // We checked earlier that this was an outbound connection
                _ => unreachable!(),
            }

            self.connections.insert(endpoint.to_string(), meta);

            // Notify subscribers of success
            subscribers.broadcast(ConnectionManagerNotification::Connected {
                endpoint: endpoint.to_string(),
            });
        } else {
            let reconnection_attempts = match meta.extended_metadata {
                ConnectionMetadataExt::Outbound {
                    ref mut reconnecting,
                    ref mut retry_frequency,
                    ref mut last_connection_attempt,
                    ref mut reconnection_attempts,
                } => {
                    *reconnecting = true;
                    *retry_frequency = min(*retry_frequency * 2, self.maximum_retry_frequency);
                    *last_connection_attempt = Instant::now();
                    *reconnection_attempts += 1;

                    *reconnection_attempts
                }
                // We checked earlier that this was an outbound connection
                _ => unreachable!(),
            };

            self.connections.insert(endpoint.to_string(), meta);

            // Notify subscribers of reconnection failure
            subscribers.broadcast(ConnectionManagerNotification::ReconnectionFailed {
                endpoint: endpoint.to_string(),
                attempts: reconnection_attempts,
            });
        }
        Ok(())
    }

    fn connection_metadata(&self) -> &HashMap<String, ConnectionMetadata> {
        &self.connections
    }

    fn connection_metadata_mut(&mut self) -> &mut HashMap<String, ConnectionMetadata> {
        &mut self.connections
    }

    fn matrix_sender(&self) -> U {
        self.matrix_sender.clone()
    }
}

fn handle_request<T: ConnectionMatrixLifeCycle, U: ConnectionMatrixSender>(
    req: CmRequest,
    state: &mut ConnectionManagerState<T, U>,
    subscribers: &mut SubscriberMap,
    authorizer: &dyn Authorizer,
    internal_sender: Sender<CmMessage>,
) {
    match req {
        CmRequest::RequestOutboundConnection {
            endpoint,
            sender,
            connection_id,
        } => state.add_outbound_connection(
            &endpoint,
            connection_id,
            sender,
            internal_sender,
            authorizer,
        ),
        CmRequest::RemoveConnection { endpoint, sender } => {
            let response = state
                .remove_connection(&endpoint)
                .map(|meta_opt| meta_opt.map(|meta| meta.endpoint().to_owned()));

            if sender.send(response).is_err() {
                warn!("connector dropped before receiving result of remove connection");
            }
        }
        CmRequest::ListConnections { sender } => {
            if sender
                .send(Ok(state
                    .connection_metadata()
                    .iter()
                    .map(|(key, _)| key.to_string())
                    .collect()))
                .is_err()
            {
                warn!("connector dropped before receiving result of list connections");
            }
        }
        CmRequest::AddInboundConnection { sender, connection } => {
            state.add_inbound_connection(connection, sender, internal_sender, authorizer)
        }
        CmRequest::Subscribe { sender, callback } => {
            let subscriber_id = subscribers.add_subscriber(callback);
            if sender.send(Ok(subscriber_id)).is_err() {
                warn!("connector dropped before receiving result of remove connection");
            }
        }
        CmRequest::Unsubscribe {
            sender,
            subscriber_id,
        } => {
            subscribers.remove_subscriber(subscriber_id);
            if sender.send(Ok(())).is_err() {
                warn!("connector dropped before receiving result of remove connection");
            }
        }
    };
}

fn handle_auth_result<T: ConnectionMatrixLifeCycle, U: ConnectionMatrixSender>(
    auth_result: AuthResult,
    state: &mut ConnectionManagerState<T, U>,
    subscribers: &mut SubscriberMap,
) {
    match auth_result {
        AuthResult::Outbound {
            endpoint,
            sender,
            auth_result,
        } => {
            let res = state.on_outbound_authorization_complete(endpoint, auth_result);
            if sender.send(res).is_err() {
                warn!("connector dropped before receiving result of connection authorization");
            }
        }
        AuthResult::Inbound {
            endpoint,
            sender,
            auth_result,
        } => {
            let res = state.on_inbound_authorization_complete(endpoint, auth_result, subscribers);
            if sender.send(res).is_err() {
                warn!("connector dropped before receiving result of connection authorization");
            }
        }
    }
}

fn send_heartbeats<T: ConnectionMatrixLifeCycle, U: ConnectionMatrixSender>(
    state: &mut ConnectionManagerState<T, U>,
    subscribers: &mut SubscriberMap,
) {
    let heartbeat_message = match create_heartbeat() {
        Ok(h) => h,
        Err(err) => {
            error!("Failed to create heartbeat message: {:?}", err);
            return;
        }
    };

    let matrix_sender = state.matrix_sender();
    let mut reconnections = vec![];
    for (endpoint, metadata) in state.connection_metadata_mut().iter_mut() {
        match metadata.extended_metadata {
            ConnectionMetadataExt::Outbound {
                reconnecting,
                retry_frequency,
                last_connection_attempt,
                ..
            } => {
                // if connection is already attempting reconnection, call reconnect
                if reconnecting {
                    if last_connection_attempt.elapsed().as_secs() > retry_frequency {
                        reconnections.push(endpoint.to_string());
                    }
                } else {
                    trace!("Sending heartbeat to {}", endpoint);
                    if let Err(err) = matrix_sender
                        .send(metadata.connection_id.clone(), heartbeat_message.clone())
                    {
                        error!(
                            "failed to send heartbeat: {:?} attempting reconnection",
                            err
                        );

                        subscribers.broadcast(ConnectionManagerNotification::Disconnected {
                            endpoint: endpoint.clone(),
                        });
                        reconnections.push(endpoint.to_string());
                    }
                }
            }
            ConnectionMetadataExt::Inbound {
                ref mut disconnected,
            } => {
                trace!("Sending heartbeat to {}", endpoint);
                if let Err(err) =
                    matrix_sender.send(metadata.connection_id.clone(), heartbeat_message.clone())
                {
                    error!(
                        "failed to send heartbeat: {:?} attempting reconnection",
                        err
                    );

                    if !*disconnected {
                        *disconnected = true;
                        subscribers.broadcast(ConnectionManagerNotification::Disconnected {
                            endpoint: endpoint.clone(),
                        });
                    }
                } else {
                    *disconnected = false;
                }
            }
        }
    }

    for endpoint in reconnections {
        if let Err(err) = state.reconnect(&endpoint, subscribers) {
            error!("Reconnection attempt to {} failed: {:?}", endpoint, err);
        }
    }
}

fn create_heartbeat() -> Result<Vec<u8>, ConnectionManagerError> {
    let heartbeat = NetworkHeartbeat::new().write_to_bytes().map_err(|_| {
        ConnectionManagerError::HeartbeatError("cannot create NetworkHeartbeat message".to_string())
    })?;
    let mut heartbeat_message = NetworkMessage::new();
    heartbeat_message.set_message_type(NetworkMessageType::NETWORK_HEARTBEAT);
    heartbeat_message.set_payload(heartbeat);
    let heartbeat_bytes = heartbeat_message.write_to_bytes().map_err(|_| {
        ConnectionManagerError::HeartbeatError("cannot create NetworkMessage".to_string())
    })?;
    Ok(heartbeat_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::mpsc;

    use crate::mesh::Mesh;
    use crate::network::auth2::tests::negotiation_connection_auth;
    use crate::network::auth2::AuthorizationPool;
    use crate::transport::inproc::InprocTransport;
    use crate::transport::socket::TcpTransport;

    #[test]
    fn test_connection_manager_startup_and_shutdown() {
        let mut transport = Box::new(InprocTransport::default());
        transport.listen("inproc://test").unwrap();
        let mesh = Mesh::new(512, 128);

        let mut cm = ConnectionManager::new(
            Box::new(NoopAuthorizer::new("test_identity")),
            mesh.get_life_cycle(),
            mesh.get_sender(),
            transport,
            None,
            None,
        );

        cm.start().unwrap();
        cm.shutdown_signaler().unwrap().shutdown();
        cm.await_shutdown();
    }

    #[test]
    fn test_add_connection_request() {
        let mut transport = Box::new(InprocTransport::default());
        let mut listener = transport.listen("inproc://test").unwrap();

        thread::spawn(move || {
            listener.accept().unwrap();
        });

        let mesh = Mesh::new(512, 128);
        let mut cm = ConnectionManager::new(
            Box::new(NoopAuthorizer::new("test_identity")),
            mesh.get_life_cycle(),
            mesh.get_sender(),
            transport,
            None,
            None,
        );
        let connector = cm.start().unwrap();

        connector
            .request_connection("inproc://test", "test_id")
            .expect("A connection could not be created");

        cm.shutdown_signaler().unwrap().shutdown();
        cm.await_shutdown();
    }

    /// Test that adding the same connection twice is an idempotent operation
    #[test]
    fn test_mutiple_add_connection_requests() {
        let mut transport = Box::new(InprocTransport::default());
        let mut listener = transport.listen("inproc://test").unwrap();

        thread::spawn(move || {
            listener.accept().unwrap();
        });

        let mesh = Mesh::new(512, 128);
        let mut cm = ConnectionManager::new(
            Box::new(NoopAuthorizer::new("test_identity")),
            mesh.get_life_cycle(),
            mesh.get_sender(),
            transport,
            None,
            None,
        );
        let connector = cm.start().unwrap();

        connector
            .request_connection("inproc://test", "test_id")
            .expect("A connection could not be created");

        connector
            .request_connection("inproc://test", "test_id")
            .expect("A connection could not be re-requested");

        cm.shutdown_signaler().unwrap().shutdown();
        cm.await_shutdown();
    }

    /// Test that heartbeats are correctly sent to inproc connections
    #[test]
    fn test_heartbeat_inproc() {
        let mut transport = Box::new(InprocTransport::default());
        let mut listener = transport.listen("inproc://test").unwrap();
        let mesh = Mesh::new(512, 128);
        let mesh_clone = mesh.clone();

        thread::spawn(move || {
            let conn = listener.accept().unwrap();
            mesh_clone.add(conn, "test_id".to_string()).unwrap();
        });

        let mut cm = ConnectionManager::new(
            Box::new(NoopAuthorizer::new("test_identity")),
            mesh.get_life_cycle(),
            mesh.get_sender(),
            transport,
            Some(1),
            None,
        );
        let connector = cm.start().unwrap();

        connector
            .request_connection("inproc://test", "test_id")
            .expect("A connection could not be created");

        // Verify mesh received heartbeat

        let envelope = mesh.recv().unwrap();
        let heartbeat: NetworkMessage = protobuf::parse_from_bytes(&envelope.payload()).unwrap();
        assert_eq!(
            heartbeat.get_message_type(),
            NetworkMessageType::NETWORK_HEARTBEAT
        );

        cm.shutdown_signaler().unwrap().shutdown();
        cm.await_shutdown();
    }

    /// Test that heartbeats are correctly sent to tcp connections
    #[test]
    fn test_heartbeat_raw_tcp() {
        let mut transport = Box::new(TcpTransport::default());
        let mut listener = transport.listen("tcp://localhost:0").unwrap();
        let endpoint = listener.endpoint();

        let mesh = Mesh::new(512, 128);

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mesh = Mesh::new(512, 128);
            let conn = listener.accept().unwrap();
            mesh.add(conn, "test_id".to_string()).unwrap();

            negotiation_connection_auth(&mesh, "test_id", "some-peer");

            // Verify mesh received heartbeat

            let envelope = mesh.recv().unwrap();
            let heartbeat: NetworkMessage =
                protobuf::parse_from_bytes(&envelope.payload()).unwrap();
            assert_eq!(
                heartbeat.get_message_type(),
                NetworkMessageType::NETWORK_HEARTBEAT
            );

            tx.send(()).expect("Could not send completion signal");

            mesh.shutdown_signaler().shutdown();
        });

        let authorization_pool = AuthorizationPool::new("test_identity".into())
            .expect("Unable to create authorization pool");
        let mut cm = ConnectionManager::new(
            Box::new(authorization_pool.pool_authorizer()),
            mesh.get_life_cycle(),
            mesh.get_sender(),
            transport,
            None,
            None,
        );
        let connector = cm.start().unwrap();

        let identity = connector
            .request_connection(&endpoint, "test_id")
            .expect("A connection could not be created");

        assert_eq!("some-peer", identity);

        // wait for completion
        rx.recv().expect("Did not receive completion signal");

        cm.shutdown_signaler().unwrap().shutdown();
        cm.await_shutdown();
        authorization_pool.shutdown_and_await();
    }

    #[test]
    fn test_remove_connection() {
        let mut transport = Box::new(TcpTransport::default());
        let mut listener = transport.listen("tcp://localhost:0").unwrap();
        let endpoint = listener.endpoint();
        let mesh = Mesh::new(512, 128);

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mesh = Mesh::new(512, 128);
            let conn = listener.accept().unwrap();
            mesh.add(conn, "test_id".to_string()).unwrap();
            negotiation_connection_auth(&mesh, "test_id", "some-peer");

            // wait for completion
            rx.recv().expect("Did not receive completion signal");

            mesh.shutdown_signaler().shutdown();
        });

        let authorization_pool = AuthorizationPool::new("test_identity".into())
            .expect("Unable to create authorization pool");
        let mut cm = ConnectionManager::new(
            Box::new(authorization_pool.pool_authorizer()),
            mesh.get_life_cycle(),
            mesh.get_sender(),
            transport,
            None,
            None,
        );
        let connector = cm.start().unwrap();

        connector
            .request_connection(&endpoint, "test_id")
            .expect("A connection could not be created");

        assert_eq!(
            vec![endpoint.clone()],
            connector
                .list_connections()
                .expect("Unable to list connections")
        );

        let endpoint_removed = connector
            .remove_connection(&endpoint)
            .expect("Unable to remove connection");

        assert_eq!(Some(endpoint.clone()), endpoint_removed);

        assert!(connector
            .list_connections()
            .expect("Unable to list connections")
            .is_empty());

        tx.send(()).expect("Could not send completion signal");

        cm.shutdown_signaler().unwrap().shutdown();
        cm.await_shutdown();
        authorization_pool.shutdown_and_await();
    }

    #[test]
    fn test_remove_nonexistent_connection() {
        let transport = Box::new(TcpTransport::default());
        let mesh = Mesh::new(512, 128);

        let mut cm = ConnectionManager::new(
            Box::new(NoopAuthorizer::new("test_identity")),
            mesh.get_life_cycle(),
            mesh.get_sender(),
            transport,
            None,
            None,
        );
        let connector = cm.start().unwrap();

        let endpoint_removed = connector
            .remove_connection("tcp://localhost:5000")
            .expect("Unable to remove connection");

        assert_eq!(None, endpoint_removed);
        cm.shutdown_signaler().unwrap().shutdown();
        cm.await_shutdown();
    }

    /// test_reconnect_raw_tcp
    ///
    /// Test that if a connection disconnects, the connection manager will detect the connection
    /// has disconnected by trying to send a heartbeat. Then connection manger will try to
    /// reconnect to the endpoint.
    #[test]
    fn test_reconnect_raw_tcp() {
        let mut transport = Box::new(TcpTransport::default());
        let mut listener = transport
            .listen("tcp://localhost:0")
            .expect("Cannot listen for connections");
        let endpoint = listener.endpoint();
        let mesh1 = Mesh::new(512, 128);

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            // accept incoming connection and add it to mesh2
            let mesh2 = Mesh::new(512, 128);
            let conn = listener.accept().expect("Cannot accept connection");
            mesh2
                .add(conn, "test_id".to_string())
                .expect("Cannot add connection to mesh");

            negotiation_connection_auth(&mesh2, "test_id", "some-peer");

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
                .remove(&"test_id".to_string())
                .expect("Cannot remove connection from mesh");
            connection
                .disconnect()
                .expect("Connection failed to disconnect");

            // wait for reconnection attempt
            listener.accept().expect("Unable to accept connection");

            // wait for completion
            rx.recv().expect("Did not receive completion signal");

            mesh2.shutdown_signaler().shutdown();
        });

        let authorization_pool = AuthorizationPool::new("test_identity".into())
            .expect("Unable to create authorization pool");
        let mut cm = ConnectionManager::new(
            Box::new(authorization_pool.pool_authorizer()),
            mesh1.get_life_cycle(),
            mesh1.get_sender(),
            transport,
            Some(1),
            None,
        );
        let connector = cm.start().expect("Unable to start ConnectionManager");

        connector
            .request_connection(&endpoint, "test_id")
            .expect("Unable to request connection");

        let mut subscriber = connector
            .subscription_iter()
            .expect("Cannot get subscriber");

        // receive reconnecting attempt
        let reconnecting_notification = subscriber
            .next()
            .expect("Cannot get message from subscriber");
        assert!(
            reconnecting_notification
                == ConnectionManagerNotification::Disconnected {
                    endpoint: endpoint.clone(),
                }
        );

        // receive successful reconnect attempt
        let reconnection_notification = subscriber
            .next()
            .expect("Cannot get message from subscriber");
        assert!(
            reconnection_notification
                == ConnectionManagerNotification::Connected {
                    endpoint: endpoint.clone(),
                }
        );

        tx.send(()).expect("Could not send completion signal");

        cm.shutdown_signaler().unwrap().shutdown();
        cm.await_shutdown();
        authorization_pool.shutdown_and_await();
    }

    /// Test that an inbound connection may be added to the connection manager
    /// This test does the following:
    /// 1. Add an inbound connection to a connection manager
    /// 2. Notify inbound listeners
    /// 3. The connection can be removed by its reported remote endpoint
    #[test]
    fn test_inbound_connection() {
        let mut transport = InprocTransport::default();
        let mut listener = transport
            .listen("inproc://test_inbound_connection")
            .expect("Cannot listen for connections");

        let mesh = Mesh::new(512, 128);
        let mut cm = ConnectionManager::new(
            Box::new(NoopAuthorizer::new("test_identity")),
            mesh.get_life_cycle(),
            mesh.get_sender(),
            Box::new(transport.clone()),
            Some(1),
            None,
        );

        let (conn_tx, conn_rx) = mpsc::channel();

        let jh = thread::spawn(move || {
            let _connection = transport
                .connect("inproc://test_inbound_connection")
                .unwrap();

            // block until done
            conn_rx.recv().unwrap();
        });
        let connector = cm.start().expect("Unable to start ConnectionManager");

        let mut subscriber = connector
            .subscription_iter()
            .expect("Cannot get subscriber");

        let connection = listener.accept().unwrap();
        connector
            .add_inbound_connection(connection)
            .expect("Unable to add inbound connection");

        let notification = subscriber
            .next()
            .expect("Cannot get message from subscriber");
        if let ConnectionManagerNotification::InboundConnection { endpoint, .. } = notification {
            assert_eq!("inproc://test_inbound_connection", &endpoint);
        } else {
            panic!("Incorrect notification received: {:?}", notification);
        }

        let connection_endpoints = connector.list_connections().unwrap();
        assert_eq!(
            vec!["inproc://test_inbound_connection".to_string()],
            connection_endpoints
        );

        connector
            .remove_connection("inproc://test_inbound_connection")
            .unwrap();
        let connection_endpoints = connector.list_connections().unwrap();
        assert!(connection_endpoints.is_empty());

        conn_tx.send(()).unwrap();
        jh.join().unwrap();

        cm.shutdown_signaler().unwrap().shutdown();
        cm.await_shutdown();
    }

    /// Test that an inbound tcp connection can be add and removed from the network.o
    ///
    /// This connection requires negotiating the connection authorization handshake.
    #[test]
    fn test_inbound_tcp_connection() {
        let mut transport = Box::new(TcpTransport::default());
        let mut listener = transport
            .listen("tcp://localhost:0")
            .expect("Cannot listen for tcp connections");
        let endpoint = listener.endpoint();

        let mesh = Mesh::new(512, 128);
        let authorization_pool = AuthorizationPool::new("test_identity".into())
            .expect("Unable to create authorization pool");
        let mut cm = ConnectionManager::new(
            Box::new(authorization_pool.pool_authorizer()),
            mesh.get_life_cycle(),
            mesh.get_sender(),
            // The transport on this end doesn't matter for this test
            Box::new(InprocTransport::default()),
            Some(1),
            None,
        );

        let (conn_tx, conn_rx) = mpsc::channel();
        let server_endpoint = endpoint.clone();
        let jh = thread::spawn(move || {
            let mesh = Mesh::new(512, 128);
            let connection = transport.connect(&server_endpoint).unwrap();

            mesh.add(connection, "test_id".into())
                .expect("Unable to add to remote mesh");

            negotiation_connection_auth(&mesh, "test_id", "inbound-identity");

            // block until done
            conn_rx.recv().unwrap();
            mesh.shutdown_signaler().shutdown();
        });
        let connector = cm.start().expect("Unable to start ConnectionManager");

        let mut subscriber = connector
            .subscription_iter()
            .expect("Cannot get subscriber");

        let connection = listener.accept().unwrap();
        let remote_endpoint = connection.remote_endpoint();
        connector
            .add_inbound_connection(connection)
            .expect("Unable to add inbound connection");

        let notification = subscriber
            .next()
            .expect("Cannot get message from subscriber");

        if let ConnectionManagerNotification::InboundConnection { ref identity, .. } = &notification
        {
            assert_eq!(identity, "inbound-identity");
        } else {
            panic!(
                "Did not receive the correct notification: {:?}",
                notification
            );
        }

        let connection_endpoints = connector.list_connections().unwrap();
        assert_eq!(vec![remote_endpoint.clone()], connection_endpoints);

        connector.remove_connection(&remote_endpoint).unwrap();
        let connection_endpoints = connector.list_connections().unwrap();
        assert!(connection_endpoints.is_empty());

        conn_tx.send(()).unwrap();
        jh.join().unwrap();

        cm.shutdown_signaler().unwrap().shutdown();
        cm.await_shutdown();
        authorization_pool.shutdown_and_await();
    }

    struct NoopAuthorizer {
        authorized_id: String,
    }

    impl NoopAuthorizer {
        fn new(id: &str) -> Self {
            Self {
                authorized_id: id.to_string(),
            }
        }
    }

    impl Authorizer for NoopAuthorizer {
        fn authorize_connection(
            &self,
            connection_id: String,
            connection: Box<dyn Connection>,
            callback: AuthorizerCallback,
        ) -> Result<(), AuthorizerError> {
            (*callback)(AuthorizationResult::Authorized {
                connection_id,
                connection,
                identity: self.authorized_id.clone(),
            })
            .map_err(|err| AuthorizerError(format!("Unable to return result: {}", err)))
        }
    }
}
