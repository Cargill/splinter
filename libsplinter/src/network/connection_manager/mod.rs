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

use std::cmp::min;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::time::Instant;

use uuid::Uuid;

pub use error::{AuthorizerError, ConnectionManagerError};
pub use notification::ConnectionManagerNotification;
use protobuf::Message;

use crate::protos::network::{NetworkHeartbeat, NetworkMessage, NetworkMessageType};
use crate::threading::pacemaker;
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

/// Responsible for broadcasting connection manager notifications.
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

/// Messages handled by the connection manager.
enum CmMessage {
    Shutdown,
    Request(CmRequest),
    AuthResult(AuthResult),
    SendHeartbeats,
}

/// CmMessages sent by a Connector.
enum CmRequest {
    RequestOutboundConnection {
        endpoint: String,
        connection_id: String,
        sender: Sender<Result<(), ConnectionManagerError>>,
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

/// Messages sent to ConnectionState to report on the status of a connection
/// authorization attempt.
enum AuthResult {
    Outbound {
        endpoint: String,
        auth_result: AuthorizationResult,
    },
    Inbound {
        endpoint: String,
        auth_result: AuthorizationResult,
    },
}

pub struct ConnectionManagerBuilder<T, U> {
    authorizer: Option<Box<dyn Authorizer + Send>>,
    life_cycle: Option<T>,
    matrix_sender: Option<U>,
    transport: Option<Box<dyn Transport + Send>>,
    heartbeat_interval: u64,
    maximum_retry_frequency: u64,
}

impl<T, U> Default for ConnectionManagerBuilder<T, U> {
    fn default() -> Self {
        Self {
            authorizer: None,
            life_cycle: None,
            matrix_sender: None,
            transport: None,
            heartbeat_interval: DEFAULT_HEARTBEAT_INTERVAL,
            maximum_retry_frequency: DEFAULT_MAXIMUM_RETRY_FREQUENCY,
        }
    }
}

/// Constructs new `ConnectionManager` instances.
///
/// This builder is used to construct new connection manager instances.  A connection manager
/// requires an authorizer, used to authorize connections, a connection matrix life-cycle, for
/// adding and removing connections from a connection matrix, a connection matrix sender, for
/// sending messages using a connection matrix.  It also has several optional configuration values,
/// such as heartbeat interval and the maximum retry frequency.
impl<T, U> ConnectionManagerBuilder<T, U>
where
    T: ConnectionMatrixLifeCycle + 'static,
    U: ConnectionMatrixSender + 'static,
{
    /// Construct a new builder.
    pub fn new() -> Self {
        Default::default()
    }

    /// Set the authorizer instance to use with the resulting connection manager.
    ///
    /// All connections managed by the resulting instance will be passed through the authorizer
    /// before being considered fully connected.
    pub fn with_authorizer(mut self, authorizer: Box<dyn Authorizer + Send>) -> Self {
        self.authorizer = Some(authorizer);
        self
    }

    /// Set the connection matrix life-cycle for the resulting connection manager.
    ///
    /// All connections managed by the resulting instance will be added or removed from the given
    /// `ConnectionMatrixLifeCycle`.
    pub fn with_matrix_life_cycle(mut self, life_cycle: T) -> Self {
        self.life_cycle = Some(life_cycle);
        self
    }

    /// Set the connection matrix sender for the resulting connection manager.
    ///
    /// All heartbeat messages will be sent using the given `ConnectionMatrixSender`.
    pub fn with_matrix_sender(mut self, matrix_sender: U) -> Self {
        self.matrix_sender = Some(matrix_sender);
        self
    }

    /// Set the transport for the resulting connection manager.
    ///
    /// All requested outbound connections will be created using the given `Transport` instance.
    pub fn with_transport(mut self, transport: Box<dyn Transport + Send>) -> Self {
        self.transport = Some(transport);
        self
    }

    /// Set the optional heartbeat interval for the resulting connection manager.
    pub fn with_heartbeat_interval(mut self, interval: u64) -> Self {
        self.heartbeat_interval = interval;
        self
    }

    /// Set the optional maximum retry frequency for the resulting connection manager.
    ///
    /// All outbound connections that are lost while managed by the resulting instance will be
    /// retried up to this maximum.
    pub fn with_maximum_retry_frequency(mut self, frequency: u64) -> Self {
        self.maximum_retry_frequency = frequency;
        self
    }

    /// Create a started connection manager instance.
    ///
    /// This function creates and starts a `ConnectionManager` instance, which includes a
    /// background thread for managing the instance's state.
    ///
    /// # Errors
    ///
    /// A `ConnectionManagerError` is returned if a required property is not set or the background
    /// thread fails to start.
    pub fn start(mut self) -> Result<ConnectionManager, ConnectionManagerError> {
        let (sender, recv) = channel();
        let heartbeat = self.heartbeat_interval;
        let retry_frequency = self.maximum_retry_frequency;

        let authorizer = self
            .authorizer
            .take()
            .ok_or_else(|| ConnectionManagerError::StartUpError("No authorizer provided".into()))?;

        let transport = self
            .transport
            .take()
            .ok_or_else(|| ConnectionManagerError::StartUpError("No transport provided".into()))?;

        let matrix_sender = self.matrix_sender.take().ok_or_else(|| {
            ConnectionManagerError::StartUpError("No matrix sender provided".into())
        })?;
        let life_cycle = self.life_cycle.take().ok_or_else(|| {
            ConnectionManagerError::StartUpError("No matrix life cycle provided".into())
        })?;

        let resender = sender.clone();
        let join_handle = thread::Builder::new()
            .name("Connection Manager".into())
            .spawn(move || {
                let mut state = ConnectionManagerState::new(
                    life_cycle,
                    matrix_sender,
                    transport,
                    retry_frequency,
                );
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
                        Ok(CmMessage::SendHeartbeats) => send_heartbeats(
                            &mut state,
                            &mut subscribers,
                            &*authorizer,
                            resender.clone(),
                        ),
                        Err(_) => {
                            warn!("All senders have disconnected");
                            break;
                        }
                    }
                }
            })?;

        debug!(
            "Starting connection manager pacemaker with interval of {}s",
            heartbeat
        );
        let pacemaker = pacemaker::Pacemaker::builder()
            .with_interval(heartbeat)
            .with_sender(sender.clone())
            .with_message_factory(|| CmMessage::SendHeartbeats)
            .start()
            .map_err(|err| ConnectionManagerError::StartUpError(err.to_string()))?;

        Ok(ConnectionManager {
            join_handle,
            pacemaker,
            sender,
        })
    }
}

/// Creates, manages, and maintains connections. A connection manager
/// guarantees that the connections it creates will be maintained via
/// reconnections. This is not true for external connections.
pub struct ConnectionManager {
    pacemaker: pacemaker::Pacemaker,
    join_handle: thread::JoinHandle<()>,
    sender: Sender<CmMessage>,
}

impl ConnectionManager {
    /// Construct a new `ConnectionManagerBuilder` for creating a new `ConnectionManager` instance.
    pub fn builder<T, U>() -> ConnectionManagerBuilder<T, U>
    where
        T: ConnectionMatrixLifeCycle + 'static,
        U: ConnectionMatrixSender + 'static,
    {
        ConnectionManagerBuilder::new()
    }

    /// Create a new connector for performing client operations on this instance's state.
    pub fn connector(&self) -> Connector {
        Connector {
            sender: self.sender.clone(),
        }
    }

    pub fn shutdown_signaler(&self) -> ShutdownSignaler {
        ShutdownSignaler {
            sender: self.sender.clone(),
            pacemaker_shutdown_signaler: self.pacemaker.shutdown_signaler(),
        }
    }

    /// Blocks until a connection manager has shutdown. This is meant to allow
    /// a separate process to shutdown the connection manager via the shutdown
    /// handle while another process waits for that process to complete.
    pub fn await_shutdown(self) {
        debug!("Shutting down connection manager pacemaker...");
        self.pacemaker.await_shutdown();
        debug!("Shutting down connection manager pacemaker (complete)");

        if let Err(err) = self.join_handle.join() {
            error!(
                "Connection manager thread did not shutdown correctly: {:?}",
                err
            );
        }
    }
}

/// Connector is a client or handle to the connection manager and is used to
/// send request to the connection manager.
#[derive(Clone)]
pub struct Connector {
    sender: Sender<CmMessage>,
}

impl Connector {
    /// Request a connection to the given endpoint.
    ///
    /// This operation is idempotent: if a connection to that endpoint already exists, a new
    /// connection is not created. On successful connection Ok is returned. The connection is not
    /// ready to use, it must complete authorization. When the connection is ready a
    /// `ConnectionManagerNotification::Connected`will be sent to subscribers.
    ///
    /// # Errors
    ///
    /// An error is returned if the connection cannot be created.
    pub fn request_connection(
        &self,
        endpoint: &str,
        connection_id: &str,
    ) -> Result<(), ConnectionManagerError> {
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

    /// Removes a connection from a connection manager.
    ///
    /// # Returns
    ///
    /// The endpoint, if the connection exists; None, otherwise.
    ///
    /// # Errors
    ///
    /// Returns a ConnectionManagerError if the query cannot be performed.
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

    /// Unsubscribe to connection manager notifications.
    ///
    /// # Errors
    ///
    /// Returns a ConnectionManagerError is the connection manager
    /// has stopped running.
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

    /// Add a new inbound connection.
    ///
    /// # Error
    ///
    /// Returns a ConnectionManagerError if the connection manager is
    /// no longer running.
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

/// Sends a shutdown signal to the connection manager.
#[derive(Clone)]
pub struct ShutdownSignaler {
    sender: Sender<CmMessage>,
    pacemaker_shutdown_signaler: pacemaker::ShutdownSignaler,
}

impl ShutdownSignaler {
    /// Signal the connection manager to shutdown.
    pub fn shutdown(self) {
        self.pacemaker_shutdown_signaler.shutdown();

        if self.sender.send(CmMessage::Shutdown).is_err() {
            warn!("Connection manager is no longer running");
        }
    }
}

/// Metadata describing a connection managed by the connection manager.
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

/// Enum describing metadata that is specific to the two different connection
/// types, outbound and inbound.
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

/// Struct describing the connection manager's internal state and handling
/// requests sent to the connection manager by its Connectors. Connection state
/// is responsible for adding, removing, and authorizing connections.
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

    /// Adds a new connection as an inbound connection.
    fn add_inbound_connection(
        &mut self,
        connection: Box<dyn Connection>,
        reply_sender: Sender<Result<(), ConnectionManagerError>>,
        internal_sender: Sender<CmMessage>,
        authorizer: &dyn Authorizer,
    ) {
        let endpoint = connection.remote_endpoint();
        let id = Uuid::new_v4().to_string();

        // add the connection to the authorization pool.
        let auth_endpoint = endpoint;
        if let Err(err) = authorizer.authorize_connection(
            id,
            connection,
            Box::new(move |auth_result| {
                internal_sender
                    .send(CmMessage::AuthResult(AuthResult::Inbound {
                        endpoint: auth_endpoint.clone(),
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
        } else if reply_sender.send(Ok(())).is_err() {
            warn!("connector dropped before receiving result of add connection");
        }
    }

    /// Adds a new outbound connection.
    fn add_outbound_connection(
        &mut self,
        endpoint: &str,
        connection_id: String,
        reply_sender: Sender<Result<(), ConnectionManagerError>>,
        internal_sender: Sender<CmMessage>,
        authorizer: &dyn Authorizer,
        subscribers: &mut SubscriberMap,
    ) {
        if let Some(connection) = self.connections.get(endpoint) {
            let identity = connection.identity().to_string();
            // if this connection not reconnecting or disconnected, send Connected
            // notification.
            match connection.extended_metadata {
                ConnectionMetadataExt::Outbound {
                    ref reconnecting, ..
                } => {
                    if !reconnecting {
                        subscribers.broadcast(ConnectionManagerNotification::Connected {
                            endpoint: endpoint.to_string(),
                            connection_id,
                            identity,
                        });
                    }
                }
                ConnectionMetadataExt::Inbound { ref disconnected } => {
                    if !disconnected {
                        subscribers.broadcast(ConnectionManagerNotification::Connected {
                            endpoint: endpoint.to_string(),
                            connection_id,
                            identity,
                        });
                    }
                }
            }

            if reply_sender.send(Ok(())).is_err() {
                warn!("connector dropped before receiving result of add connection");
            }
        } else {
            match self.transport.connect(endpoint) {
                Ok(connection) => {
                    // add the connection to the authorization pool.
                    let auth_endpoint = endpoint.to_string();
                    if let Err(err) = authorizer.authorize_connection(
                        connection_id,
                        connection,
                        Box::new(move |auth_result| {
                            internal_sender
                                .send(CmMessage::AuthResult(AuthResult::Outbound {
                                    endpoint: auth_endpoint.clone(),
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
                    } else if reply_sender.send(Ok(())).is_err() {
                        warn!("connector dropped before receiving result of add connection");
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

    /// Adds outbound connection to matrix life cycle after the connection has
    /// been authorized. These connections cannot be reconnected when dropped
    /// or lost.
    ///
    /// # Returns
    ///
    /// A string representing the Connection ID.
    ///
    /// # Errors
    ///
    /// Returns a connection manager error if the connection is unauthorized or
    /// if the life cycle fails to add the connection.
    fn on_outbound_authorization_complete(
        &mut self,
        endpoint: String,
        auth_result: AuthorizationResult,
        subscribers: &mut SubscriberMap,
    ) {
        match auth_result {
            AuthorizationResult::Authorized {
                connection_id,
                connection,
                identity,
            } => {
                if let Err(err) = self
                    .life_cycle
                    .add(connection, connection_id.clone())
                    .map_err(|err| {
                        ConnectionManagerError::ConnectionCreationError(format!("{:?}", err))
                    })
                {
                    subscribers.broadcast(ConnectionManagerNotification::FatalConnectionError {
                        endpoint,
                        error: err,
                    });

                    return;
                }

                self.connections.insert(
                    endpoint.clone(),
                    ConnectionMetadata {
                        connection_id: connection_id.to_string(),
                        identity: identity.clone(),
                        endpoint: endpoint.clone(),
                        extended_metadata: ConnectionMetadataExt::Outbound {
                            reconnecting: false,
                            retry_frequency: INITIAL_RETRY_FREQUENCY,
                            last_connection_attempt: Instant::now(),
                            reconnection_attempts: 0,
                        },
                    },
                );

                subscribers.broadcast(ConnectionManagerNotification::Connected {
                    endpoint,
                    connection_id,
                    identity,
                });
            }
            AuthorizationResult::Unauthorized { connection_id, .. } => {
                if self.connections.remove(&endpoint).is_some() {
                    warn!("Reconnecting connection failed authorization");
                }
                // If the connection is unauthorized, notify subscriber this is a bad connection
                // and will not be added.
                subscribers.broadcast(ConnectionManagerNotification::FatalConnectionError {
                    endpoint,
                    error: ConnectionManagerError::Unauthorized(connection_id),
                });
            }
        }
    }

    /// Adds inbound connection to matrix life cycle after it has been authorized.
    ///
    /// # Errors
    ///
    /// Returns a connection manager error if the connection is unauthorized or
    /// if the life cycle fails to add the connection.
    fn on_inbound_authorization_complete(
        &mut self,
        endpoint: String,
        auth_result: AuthorizationResult,
        subscribers: &mut SubscriberMap,
    ) {
        match auth_result {
            AuthorizationResult::Authorized {
                connection_id,
                connection,
                identity,
            } => {
                if let Err(err) = self
                    .life_cycle
                    .add(connection, connection_id.clone())
                    .map_err(|err| {
                        ConnectionManagerError::ConnectionCreationError(format!("{:?}", err))
                    })
                {
                    subscribers.broadcast(ConnectionManagerNotification::FatalConnectionError {
                        endpoint,
                        error: err,
                    });
                    return;
                }

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
            }
            AuthorizationResult::Unauthorized { connection_id, .. } => {
                // If the connection is unauthorized, notify subscriber this is a bad connection
                // and will not be added.
                subscribers.broadcast(ConnectionManagerNotification::FatalConnectionError {
                    endpoint,
                    error: ConnectionManagerError::Unauthorized(connection_id),
                });
            }
        }
    }

    /// Removes connection from state.
    ///
    /// # Returns
    ///
    /// Returns metadata for the connection if available.
    ///
    /// # Errors
    ///
    /// ConnectionManagerError if the connection cannot be removed from
    /// the matrix life cycle.
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

    /// Handles reconnection operation.
    ///
    /// # Errors
    ///
    /// Returns ConnectionManagerError if reconnection operation fails due to
    /// an error caused by the matrix life cycle.
    fn reconnect(
        &mut self,
        endpoint: &str,
        subscribers: &mut SubscriberMap,
        authorizer: &dyn Authorizer,
        internal_sender: Sender<CmMessage>,
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

            let auth_endpoint = endpoint.to_string();
            if let Err(err) = authorizer.authorize_connection(
                meta.connection_id,
                connection,
                Box::new(move |auth_result| {
                    internal_sender
                        .send(CmMessage::AuthResult(AuthResult::Outbound {
                            endpoint: auth_endpoint.clone(),
                            auth_result,
                        }))
                        .map_err(Box::from)
                }),
            ) {
                error!("Error authorizing {}: {}", endpoint, err);
            }
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
            let identity = meta.identity.to_string();
            self.connections.insert(endpoint.to_string(), meta);

            // Notify subscribers of reconnection failure
            subscribers.broadcast(ConnectionManagerNotification::NonFatalConnectionError {
                endpoint: endpoint.to_string(),
                attempts: reconnection_attempts,
                identity,
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

/// Auxiliary method for handling requests sent to the connection manager.
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
            subscribers,
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

/// Auxiliary method for handling CmManager::AuthResult messages sent to connection manager.
fn handle_auth_result<T: ConnectionMatrixLifeCycle, U: ConnectionMatrixSender>(
    auth_result: AuthResult,
    state: &mut ConnectionManagerState<T, U>,
    subscribers: &mut SubscriberMap,
) {
    match auth_result {
        AuthResult::Outbound {
            endpoint,
            auth_result,
        } => {
            state.on_outbound_authorization_complete(endpoint, auth_result, subscribers);
        }
        AuthResult::Inbound {
            endpoint,
            auth_result,
        } => {
            state.on_inbound_authorization_complete(endpoint, auth_result, subscribers);
        }
    }
}

/// Auxiliary method for handling CmManager::SendHeartBeats messages sent to
/// connection manager.
fn send_heartbeats<T: ConnectionMatrixLifeCycle, U: ConnectionMatrixSender>(
    state: &mut ConnectionManagerState<T, U>,
    subscribers: &mut SubscriberMap,
    authorizer: &dyn Authorizer,
    internal_sender: Sender<CmMessage>,
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
                            "Outbound: failed to send heartbeat to {}: {:?} attempting reconnection",
                            endpoint, err
                        );

                        subscribers.broadcast(ConnectionManagerNotification::Disconnected {
                            endpoint: endpoint.clone(),
                            identity: metadata.identity.to_string(),
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
                        "Inbound: failed to send heartbeat to {}: {:?} ",
                        endpoint, err,
                    );

                    if !*disconnected {
                        *disconnected = true;
                        subscribers.broadcast(ConnectionManagerNotification::Disconnected {
                            endpoint: endpoint.clone(),
                            identity: metadata.identity.to_string(),
                        });
                    }
                } else {
                    *disconnected = false;
                }
            }
        }
    }

    for endpoint in reconnections {
        if let Err(err) = state.reconnect(
            &endpoint,
            subscribers,
            &*authorizer,
            internal_sender.clone(),
        ) {
            error!("Reconnection attempt to {} failed: {:?}", endpoint, err);
        }
    }
}

/// Creates NetworkHeartbeat message and serializes it into a byte array.
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
    use crate::network::auth::tests::negotiation_connection_auth;
    use crate::network::auth::AuthorizationManager;
    use crate::transport::inproc::InprocTransport;
    use crate::transport::socket::TcpTransport;

    #[test]
    fn test_connection_manager_startup_and_shutdown() {
        let mut transport = Box::new(InprocTransport::default());
        transport.listen("inproc://test").unwrap();
        let mesh = Mesh::new(512, 128);

        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(NoopAuthorizer::new("test_identity")))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(transport)
            .start()
            .expect("Unable to start Connection Manager");

        cm.shutdown_signaler().shutdown();
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
        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(NoopAuthorizer::new("test_identity")))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(transport)
            .start()
            .expect("Unable to start Connection Manager");

        let connector = cm.connector();

        connector
            .request_connection("inproc://test", "test_id")
            .expect("A connection could not be created");

        cm.shutdown_signaler().shutdown();
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
        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(NoopAuthorizer::new("test_identity")))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(transport)
            .start()
            .expect("Unable to start Connection Manager");

        let connector = cm.connector();

        connector
            .request_connection("inproc://test", "test_id")
            .expect("A connection could not be created");

        connector
            .request_connection("inproc://test", "test_id")
            .expect("A connection could not be re-requested");

        cm.shutdown_signaler().shutdown();
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

        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(NoopAuthorizer::new("test_identity")))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(transport)
            .start()
            .expect("Unable to start Connection Manager");

        let connector = cm.connector();

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

        cm.shutdown_signaler().shutdown();
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

        let auth_mgr = AuthorizationManager::new("test_identity".into())
            .expect("Unable to create authorization pool");
        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(auth_mgr.authorization_connector()))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(transport)
            .start()
            .expect("Unable to start Connection Manager");
        let connector = cm.connector();

        connector
            .request_connection(&endpoint, "test_id")
            .expect("A connection could not be created");

        let (sub_tx, sub_rx): (
            Sender<ConnectionManagerNotification>,
            mpsc::Receiver<ConnectionManagerNotification>,
        ) = channel();
        connector.subscribe(sub_tx).expect("Unable to respond.");

        // Validate that the connection completed authorization
        let notification = sub_rx.recv().expect("Cannot receive notification");
        assert!(
            notification
                == ConnectionManagerNotification::Connected {
                    endpoint: endpoint.clone(),
                    connection_id: "test_id".to_string(),
                    identity: "some-peer".to_string()
                }
        );

        // wait for completion
        rx.recv().expect("Did not receive completion signal");

        cm.shutdown_signaler().shutdown();
        cm.await_shutdown();
        auth_mgr.shutdown_and_await();
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

        let auth_mgr = AuthorizationManager::new("test_identity".into())
            .expect("Unable to create authorization pool");
        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(auth_mgr.authorization_connector()))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(transport)
            .start()
            .expect("Unable to start Connection Manager");
        let connector = cm.connector();

        let (sub_tx, sub_rx): (
            Sender<ConnectionManagerNotification>,
            mpsc::Receiver<ConnectionManagerNotification>,
        ) = channel();
        connector.subscribe(sub_tx).expect("Unable to respond.");

        connector
            .request_connection(&endpoint, "test_id")
            .expect("A connection could not be created");

        // Validate that the connection completed authorization
        let notification = sub_rx.recv().expect("Cannot receive notification");
        assert!(
            notification
                == ConnectionManagerNotification::Connected {
                    endpoint: endpoint.clone(),
                    connection_id: "test_id".to_string(),
                    identity: "some-peer".to_string()
                }
        );

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

        cm.shutdown_signaler().shutdown();
        cm.await_shutdown();
        auth_mgr.shutdown_and_await();
    }

    #[test]
    fn test_remove_nonexistent_connection() {
        let transport = Box::new(TcpTransport::default());
        let mesh = Mesh::new(512, 128);

        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(NoopAuthorizer::new("test_identity")))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(transport)
            .start()
            .expect("Unable to start Connection Manager");

        let connector = cm.connector();

        let endpoint_removed = connector
            .remove_connection("tcp://localhost:5000")
            .expect("Unable to remove connection");

        assert_eq!(None, endpoint_removed);
        cm.shutdown_signaler().shutdown();
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
            let conn = listener.accept().expect("Unable to accept connection");
            mesh2
                .add(conn, "test_id".to_string())
                .expect("Cannot add connection to mesh");
            negotiation_connection_auth(&mesh2, "test_id", "some-peer");

            // wait for completion
            rx.recv().expect("Did not receive completion signal");

            mesh2.shutdown_signaler().shutdown();
        });

        let auth_mgr = AuthorizationManager::new("test_identity".into())
            .expect("Unable to create authorization pool");
        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(auth_mgr.authorization_connector()))
            .with_matrix_life_cycle(mesh1.get_life_cycle())
            .with_matrix_sender(mesh1.get_sender())
            .with_transport(transport)
            .start()
            .expect("Unable to start Connection Manager");
        let connector = cm.connector();

        let (sub_tx, sub_rx): (
            Sender<ConnectionManagerNotification>,
            mpsc::Receiver<ConnectionManagerNotification>,
        ) = channel();
        connector.subscribe(sub_tx).expect("Unable to respond.");

        connector
            .request_connection(&endpoint, "test_id")
            .expect("A connection could not be created");

        // Validate that the connection completed authorization
        let notification = sub_rx.recv().expect("Cannot receive notification");
        assert!(
            notification
                == ConnectionManagerNotification::Connected {
                    endpoint: endpoint.clone(),
                    connection_id: "test_id".to_string(),
                    identity: "some-peer".to_string()
                }
        );

        let (subs_tx, subs_rx) = mpsc::channel();
        connector.subscribe(subs_tx).expect("Cannot subscribe");
        let mut subscriber = subs_rx.iter();

        // receive reconnecting attempt
        let reconnecting_notification: ConnectionManagerNotification = subscriber
            .next()
            .expect("Cannot get message from subscriber");

        assert!(
            reconnecting_notification
                == ConnectionManagerNotification::Disconnected {
                    endpoint: endpoint.clone(),
                    identity: "some-peer".to_string()
                }
        );

        // receive successful reconnect attempt
        let reconnection_notification = subscriber
            .next()
            .expect("Cannot get message from subscriber");

        assert_eq!(
            reconnection_notification,
            ConnectionManagerNotification::Connected {
                endpoint: endpoint.clone(),
                connection_id: "test_id".to_string(),
                identity: "some-peer".to_string()
            }
        );

        tx.send(()).expect("Could not send completion signal");

        cm.shutdown_signaler().shutdown();
        cm.await_shutdown();
        auth_mgr.shutdown_and_await();
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

        let (conn_tx, conn_rx) = mpsc::channel();

        let mut remote_transport = transport.clone();
        let jh = thread::spawn(move || {
            let _connection = remote_transport
                .connect("inproc://test_inbound_connection")
                .unwrap();

            // block until done
            conn_rx.recv().unwrap();
        });
        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(NoopAuthorizer::new("test_identity")))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(Box::new(transport))
            .start()
            .expect("Unable to start Connection Manager");

        let connector = cm.connector();

        let (subs_tx, subs_rx) = mpsc::channel();
        connector.subscribe(subs_tx).expect("Cannot get subscriber");

        let connection = listener.accept().unwrap();
        connector
            .add_inbound_connection(connection)
            .expect("Unable to add inbound connection");

        let notification = subs_rx
            .iter()
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

        cm.shutdown_signaler().shutdown();
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
        let auth_mgr = AuthorizationManager::new("test_identity".into())
            .expect("Unable to create authorization pool");

        let (conn_tx, conn_rx) = mpsc::channel();
        let server_endpoint = endpoint.clone();
        let jh = thread::spawn(move || {
            let mesh = Mesh::new(512, 128);
            let mut transport = Box::new(TcpTransport::default());
            let connection = transport.connect(&server_endpoint).unwrap();

            mesh.add(connection, "test_id".into())
                .expect("Unable to add to remote mesh");

            negotiation_connection_auth(&mesh, "test_id", "inbound-identity");

            // block until done
            conn_rx.recv().unwrap();
            mesh.shutdown_signaler().shutdown();
        });

        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(auth_mgr.authorization_connector()))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(transport)
            .start()
            .expect("Unable to start Connection Manager");
        let connector = cm.connector();

        let (subs_tx, subs_rx) = mpsc::channel();
        connector.subscribe(subs_tx).expect("Cannot get subscriber");

        let connection = listener.accept().unwrap();
        let remote_endpoint = connection.remote_endpoint();
        connector
            .add_inbound_connection(connection)
            .expect("Unable to add inbound connection");

        let notification = subs_rx
            .iter()
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

        cm.shutdown_signaler().shutdown();
        cm.await_shutdown();
        auth_mgr.shutdown_and_await();
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
