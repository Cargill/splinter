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

//! The service network modules provides structs for managing the connections and communications
//! with services processors over connections.

mod error;
pub mod handlers;
pub mod interconnect;

use std::collections::{BTreeMap, HashMap};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::circuit::service::ServiceId;
use crate::network::connection_manager::{ConnectionManagerNotification, Connector};
use crate::protocol::service::ServiceProcessorMessage;

use self::error::ServiceConnectionAgentError;
pub use self::error::ServiceConnectionError;
pub use self::error::{
    ServiceAddInstanceError, ServiceForwardingError, ServiceRemoveInstanceError,
};

pub type SubscriberId = usize;
type Subscriber =
    Box<dyn Fn(ServiceConnectionNotification) -> Result<(), Box<dyn std::error::Error>> + Send>;

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

    fn notify_all(&mut self, notification: ServiceConnectionNotification) {
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

#[derive(Debug, Clone)]
pub enum ServiceConnectionNotification {
    Connected {
        service_id: String,
        endpoint: String,
    },
    Disconnected {
        service_id: String,
        endpoint: String,
    },
}

/// A mapping of service instances and the component responsible for it.  This can be used to add
/// or remove service connection information.
pub trait ServiceInstances {
    /// Add a service instance.
    ///
    /// This method should create an association of the service with the given component id.
    ///
    /// # Errors
    ///
    /// Returns a `ServiceAddInstanceError` if the service cannot be added.
    fn add_service_instance(
        &self,
        service_id: ServiceId,
        component_id: String,
    ) -> Result<(), ServiceAddInstanceError>;

    /// Remove a service instance.
    ///
    /// This method should remove the association of the service with the given component id.
    ///
    /// # Errors
    ///
    /// Returns a `ServiceRemoveInstanceError` if the service cannot be removed.
    fn remove_service_instance(
        &self,
        service_id: ServiceId,
        component_id: String,
    ) -> Result<(), ServiceRemoveInstanceError>;
}

/// A component that may forward the message from a service instance to either other parts of the
/// system or the network.
///
/// The implementation may require that the service is registered with the system.
pub trait ServiceMessageForwarder {
    /// Forward the given message from the service.
    ///
    /// # Errors
    ///
    /// Returns a `ServiceForwardingError` if the service is not registered, or an internal error
    /// prevents the message from being forwarded.
    fn forward(
        &self,
        service_id: &ServiceId,
        service_msg: ServiceProcessorMessage,
    ) -> Result<ForwardResult, ServiceForwardingError>;
}

/// A Forward result indicates whether or not a message has been forwarded, or should be re-routed
/// locally.
pub enum ForwardResult {
    /// The message was successully forwarded on.
    Sent,

    /// The message should be forwarded locally.
    ///
    /// This tuple includes the component id and the original attempted message.
    LocalReReroute(String, ServiceProcessorMessage),
}

/// Constructs new ServiceConnectionManager structs.
///
/// At build time, this has initialized the background threads required for running this process.
#[derive(Default)]
pub struct ServiceConnectionManagerBuilder {
    connector: Option<Connector>,
}

impl ServiceConnectionManagerBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_connector(mut self, connector: Connector) -> Self {
        self.connector = Some(connector);
        self
    }

    pub fn start(mut self) -> Result<ServiceConnectionManager, ServiceConnectionError> {
        let (tx, rx) = mpsc::channel();

        let connector = self
            .connector
            .take()
            .ok_or_else(|| ServiceConnectionError("Must provide a valid connector".into()))?;
        let subscriber_id = connector.subscribe(tx.clone()).map_err(|err| {
            ServiceConnectionError(format!(
                "Unable to subscribe to connection manager notifications: {}",
                err
            ))
        })?;

        let connector_unsubscribe = connector.clone();
        let join_handle = thread::Builder::new()
            .name("Service Connection Manager".into())
            .spawn(move || {
                let mut agent = ServiceConnectionAgent::new(rx);

                if let Err(err) = agent.run() {
                    error!("An unexpected error occurred: {}", err);
                }

                if let Err(err) = connector_unsubscribe.unsubscribe(subscriber_id) {
                    error!(
                        "Unable to unsubscribe from connection manager notifications: {}",
                        err
                    );
                }
                debug!("Service Connection Manager terminating");
            })
            .map_err(|_| ServiceConnectionError("Unable to create background thread".into()))?;

        let service_connection_mgr = ServiceConnectionManager {
            _connector: connector,
            sender: tx,
            join_handle,
        };

        Ok(service_connection_mgr)
    }
}

/// The message passed from ServiceConnectors to the ServiceConnectionAgent.
enum AgentMessage {
    ConnectionNotification(ConnectionManagerNotification),
    ListServices {
        reply_sender: Sender<Result<Vec<String>, ServiceConnectionError>>,
    },
    GetConnectionId {
        identity: String,
        reply_sender: Sender<Result<Option<String>, ServiceConnectionError>>,
    },
    GetIdentity {
        connection_id: String,
        reply_sender: Sender<Result<Option<String>, ServiceConnectionError>>,
    },
    Subscribe {
        subscriber: Subscriber,
        reply_sender: Sender<Result<SubscriberId, ServiceConnectionError>>,
    },
    Unsubscribe {
        subscriber_id: SubscriberId,
        reply_sender: Sender<Result<(), ServiceConnectionError>>,
    },
    Shutdown,
}

/// The service connection manager.
///
/// This struct provides ServiceConnectors that are used to control or receive information about
/// service connections.
pub struct ServiceConnectionManager {
    _connector: Connector,
    sender: Sender<AgentMessage>,
    join_handle: thread::JoinHandle<()>,
}

impl ServiceConnectionManager {
    /// Create a builder for a `ServiceConnectionManager`.
    ///
    /// For example:
    ///
    /// ```no_run
    /// # use splinter::mesh::Mesh;
    /// # use splinter::network::auth::AuthorizationManager;
    /// # use splinter::network::connection_manager::{Authorizer, ConnectionManager};
    /// # use splinter::service::network::ServiceConnectionManager;
    /// # use splinter::transport::inproc::InprocTransport;
    /// # let transport = InprocTransport::default();
    /// # let mesh = Mesh::new(1, 1);
    /// # let auth_mgr = AuthorizationManager::new("test_identity".into()).unwrap();
    /// # let authorizer: Box<dyn Authorizer + Send> = Box::new(auth_mgr.authorization_connector());
    /// let mut cm = ConnectionManager::builder()
    ///     .with_authorizer(authorizer)
    ///     .with_matrix_life_cycle(mesh.get_life_cycle())
    ///     .with_matrix_sender(mesh.get_sender())
    ///     .with_transport(Box::new(transport))
    ///     .start()
    ///     .expect("Unable to start Connection Manager");
    ///
    /// let connector = cm.connector();
    ///
    /// let service_connection_manager = ServiceConnectionManager::builder()
    ///     .with_connector(connector)
    ///     .start()
    ///     .unwrap();
    /// ```
    pub fn builder() -> ServiceConnectionManagerBuilder {
        ServiceConnectionManagerBuilder::new()
    }

    /// Returns a shutdown signaler.
    pub fn shutdown_signaler(&self) -> ShutdownSignaler {
        ShutdownSignaler {
            sender: self.sender.clone(),
        }
    }

    /// Wait for the internal system to shutdown.
    ///
    /// This functions blocks until the background thread has been terminated.
    pub fn await_shutdown(self) {
        if self.join_handle.join().is_err() {
            error!("Service connection manager background thread could not be joined cleanly");
        }
    }

    /// Returns a new ServiceConnector.
    pub fn service_connector(&self) -> ServiceConnector {
        ServiceConnector {
            sender: self.sender.clone(),
        }
    }
}

/// Sends a message to the background agent and blocks while waiting for a reply.
/// It injects the reply_sender into the message being sent to the agent.  This allows for the
/// usage:
///
/// ```
/// agent_msg!(self.sender, ListServices)
/// ```
/// or
/// ```
/// agent_msg!(
///     self.sender,
///     GetConnectionId {
///         identity: identity.to_string(),
///     }
/// )
/// ```
///
/// This removes the repeated error messages that are specific to the senders in this exchange, but
/// doesn't match the other uses of send/recv to warrant a From implementation on those error
/// types.
macro_rules! agent_msg {
    (@do_send $sender:expr, $rx:expr, $msg:expr) => {
        {
            $sender
                .send($msg)
                .map_err(|_| {
                    ServiceConnectionError(
                        "Service connection manager background thread terminated unexpectedly".into(),
                    )
                })?;

            $rx.recv().map_err(|_| {
                ServiceConnectionError(
                    "Service connection manager background thread terminated unexpectedly".into(),
                )
            })?
        }
    };

    ($sender:expr, $msg_type:ident) => {
        {
            let (tx, rx) = mpsc::channel();
            agent_msg!(@do_send $sender, rx,
                AgentMessage::$msg_type {
                    reply_sender: tx,
                })
        }
    };

    ($sender:expr, $msg_type:ident { $($field:ident: $value:expr,)* }) => {
        {
            let (tx, rx) = mpsc::channel();
            agent_msg!(@do_send $sender, rx,
                AgentMessage::$msg_type {
                    reply_sender: tx,
                    $($field: $value)*
                })
        }
    };
}

/// Simple macro for handling and logging the send error on a reply.
macro_rules! agent_reply {
    ($sender:expr, $value:expr) => {{
        if $sender.send($value).is_err() {
            error!("Service Connection Manager reply sender was prematurely dropped");
        }

        Ok(())
    }};
}

/// The client for modifying or interrogating service connection state.
#[derive(Clone)]
pub struct ServiceConnector {
    sender: Sender<AgentMessage>,
}

impl ServiceConnector {
    /// Returns a list of the currently connected service identities.
    pub fn list_service_connections(&self) -> Result<Vec<String>, ServiceConnectionError> {
        agent_msg!(self.sender, ListServices)
    }

    /// Return the connection id for a given service processor identity.
    pub fn get_connection_id(
        &self,
        identity: &str,
    ) -> Result<Option<String>, ServiceConnectionError> {
        agent_msg!(
            self.sender,
            GetConnectionId {
                identity: identity.to_string(),
            }
        )
    }

    /// Return service processor identity for a given connection id.
    pub fn get_identity(
        &self,
        connection_id: &str,
    ) -> Result<Option<String>, ServiceConnectionError> {
        agent_msg!(
            self.sender,
            GetIdentity {
                connection_id: connection_id.to_string(),
            }
        )
    }

    pub fn subscribe<T>(
        &self,
        subscriber: Sender<T>,
    ) -> Result<SubscriberId, ServiceConnectionError>
    where
        T: From<ServiceConnectionNotification> + Send + 'static,
    {
        agent_msg!(
            self.sender,
            Subscribe {
                subscriber: Box::new(move |notification| {
                    subscriber.send(T::from(notification)).map_err(Box::from)
                }),
            }
        )
    }

    pub fn unsubscribe(&self, subscriber_id: SubscriberId) -> Result<(), ServiceConnectionError> {
        agent_msg!(
            self.sender,
            Unsubscribe {
                subscriber_id: subscriber_id,
            }
        )
    }
}

pub struct ShutdownSignaler {
    sender: Sender<AgentMessage>,
}

impl ShutdownSignaler {
    pub fn shutdown(&self) {
        if self.sender.send(AgentMessage::Shutdown).is_err() {
            error!("Service connection manager background thread terminated unexpectedly");
        }
    }
}

impl From<ConnectionManagerNotification> for AgentMessage {
    fn from(notification: ConnectionManagerNotification) -> Self {
        AgentMessage::ConnectionNotification(notification)
    }
}

struct ServiceConnectionInfo {
    endpoint: String,
    connection_id: String,
    identity: String,
    status: ConnectionStatus,
}

enum ConnectionStatus {
    Connected,
    Disconnected,
}

struct ServiceConnectionAgent {
    services: ServiceConnectionMap,
    receiver: Receiver<AgentMessage>,
    subscribers: SubscriberMap,
}

impl ServiceConnectionAgent {
    fn new(receiver: Receiver<AgentMessage>) -> Self {
        Self {
            services: ServiceConnectionMap::new(),
            receiver,
            subscribers: SubscriberMap::new(),
        }
    }

    fn run(&mut self) -> Result<(), ServiceConnectionAgentError> {
        loop {
            match self.receiver.recv() {
                Ok(AgentMessage::ConnectionNotification(notification)) => {
                    self.handle_notification(notification)?;
                }
                Ok(AgentMessage::ListServices { reply_sender }) => {
                    self.list_services(reply_sender)?;
                }
                Ok(AgentMessage::GetConnectionId {
                    identity,
                    reply_sender,
                }) => {
                    self.get_connection_id(&identity, reply_sender)?;
                }
                Ok(AgentMessage::GetIdentity {
                    connection_id,
                    reply_sender,
                }) => {
                    self.get_identity_for_connection_id(&connection_id, reply_sender)?;
                }
                Ok(AgentMessage::Subscribe {
                    subscriber,
                    reply_sender,
                }) => self.add_subscriber(subscriber, reply_sender)?,
                Ok(AgentMessage::Unsubscribe {
                    subscriber_id,
                    reply_sender,
                }) => self.remove_subscriber(subscriber_id, reply_sender)?,
                Ok(AgentMessage::Shutdown) => break Ok(()),
                Err(_) => {
                    break Err(ServiceConnectionAgentError(
                        "Service Connection Manager was dropped prematurely".into(),
                    ))
                }
            }
        }
    }

    fn add_subscriber(
        &mut self,
        subscriber: Subscriber,
        reply_sender: Sender<Result<SubscriberId, ServiceConnectionError>>,
    ) -> Result<(), ServiceConnectionAgentError> {
        let subscriber_id = self.subscribers.add_subscriber(subscriber);
        agent_reply!(reply_sender, Ok(subscriber_id))
    }

    fn remove_subscriber(
        &mut self,
        subscriber_id: SubscriberId,
        reply_sender: Sender<Result<(), ServiceConnectionError>>,
    ) -> Result<(), ServiceConnectionAgentError> {
        self.subscribers.remove_subscriber(subscriber_id);
        agent_reply!(reply_sender, Ok(()))
    }

    fn list_services(
        &self,
        reply_sender: Sender<Result<Vec<String>, ServiceConnectionError>>,
    ) -> Result<(), ServiceConnectionAgentError> {
        agent_reply!(reply_sender, Ok(self.services.list_connection_identities()))
    }

    fn get_connection_id(
        &self,
        identity: &str,
        reply_sender: Sender<Result<Option<String>, ServiceConnectionError>>,
    ) -> Result<(), ServiceConnectionAgentError> {
        agent_reply!(
            reply_sender,
            Ok(self
                .services
                .get_connection_info(identity)
                .map(|info| info.connection_id.to_string()))
        )
    }

    fn get_identity_for_connection_id(
        &self,
        connection_id: &str,
        reply_sender: Sender<Result<Option<String>, ServiceConnectionError>>,
    ) -> Result<(), ServiceConnectionAgentError> {
        agent_reply!(
            reply_sender,
            Ok(self
                .services
                .get_connection_info_by_connection_id(connection_id)
                .map(|info| info.identity.to_string()))
        )
    }

    fn handle_notification(
        &mut self,
        notification: ConnectionManagerNotification,
    ) -> Result<(), ServiceConnectionAgentError> {
        match notification {
            ConnectionManagerNotification::InboundConnection {
                endpoint,
                connection_id,
                identity,
            } => {
                self.services.add_connection(ServiceConnectionInfo {
                    endpoint: endpoint.clone(),
                    connection_id,
                    identity: identity.clone(),
                    status: ConnectionStatus::Connected,
                });

                self.subscribers
                    .notify_all(ServiceConnectionNotification::Connected {
                        service_id: identity,
                        endpoint,
                    })
            }
            ConnectionManagerNotification::Disconnected { endpoint, .. } => {
                if let Some(info) = self.services.get_connection_info_by_endpoint_mut(&endpoint) {
                    info.status = ConnectionStatus::Disconnected;
                    self.subscribers
                        .notify_all(ServiceConnectionNotification::Disconnected {
                            service_id: info.identity.clone(),
                            endpoint,
                        });
                }
            }
            ConnectionManagerNotification::Connected { endpoint, .. } => {
                if let Some(info) = self.services.get_connection_info_by_endpoint_mut(&endpoint) {
                    info.status = ConnectionStatus::Connected;
                    self.subscribers
                        .notify_all(ServiceConnectionNotification::Connected {
                            service_id: info.identity.clone(),
                            endpoint,
                        });
                }
            }
            ConnectionManagerNotification::NonFatalConnectionError {
                endpoint, attempts, ..
            } => {
                if let Some(info) = self.services.remove_connection_by_endoint(&endpoint) {
                    error!(
                        "Failed to reconnect to service processor {} after {}] attempts; removing",
                        info.identity, attempts
                    );
                }
            }
            ConnectionManagerNotification::FatalConnectionError { endpoint, error } => {
                if let Some(info) = self.services.remove_connection_by_endoint(&endpoint) {
                    error!(
                        "Service processor {} connection failed: {}; removing",
                        info.identity, error
                    );
                }
            }
        }

        Ok(())
    }
}

struct ServiceConnectionMap {
    services: HashMap<String, ServiceConnectionInfo>,

    // indexes
    by_endpoint: BTreeMap<String, String>,
    by_connection_id: BTreeMap<String, String>,
}

impl ServiceConnectionMap {
    fn new() -> Self {
        Self {
            services: HashMap::new(),
            by_endpoint: BTreeMap::new(),
            by_connection_id: BTreeMap::new(),
        }
    }

    fn add_connection(&mut self, service_conn: ServiceConnectionInfo) {
        let identity = service_conn.identity.clone();
        self.by_endpoint
            .insert(service_conn.endpoint.clone(), identity.clone());
        self.by_connection_id
            .insert(service_conn.connection_id.clone(), identity.clone());

        self.services.insert(identity, service_conn);
    }

    fn remove_connection_by_endoint(&mut self, endpoint: &str) -> Option<ServiceConnectionInfo> {
        self.by_endpoint
            .remove(endpoint)
            .and_then(|identity| self.services.remove(&identity))
            .and_then(|info| {
                self.by_connection_id.remove(&info.connection_id);
                Some(info)
            })
    }

    fn get_connection_info(&self, identity: &str) -> Option<&ServiceConnectionInfo> {
        self.services.get(identity)
    }

    fn get_connection_info_by_connection_id(
        &self,
        identity: &str,
    ) -> Option<&ServiceConnectionInfo> {
        let identity = self.by_connection_id.get(identity)?;
        self.services.get(identity)
    }

    fn get_connection_info_by_endpoint_mut(
        &mut self,
        endpoint: &str,
    ) -> Option<&mut ServiceConnectionInfo> {
        let identity: &String = self.by_endpoint.get(endpoint)?;
        self.services.get_mut(identity)
    }

    fn list_connection_identities(&self) -> Vec<String> {
        self.services.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::mpsc;
    use std::thread;

    use crate::mesh::Mesh;
    use crate::network::connection_manager::{
        AuthorizationResult, Authorizer, AuthorizerCallback, AuthorizerError, ConnectionManager,
    };
    use crate::transport::{inproc::InprocTransport, Connection, Transport};

    impl ServiceConnectionManager {
        pub fn shutdown_and_wait(self) {
            self.shutdown_signaler().shutdown();
            self.await_shutdown();
        }
    }

    /// Test that the ServiceConnectionManager will accept an incoming connection and add it to its
    /// collection of service processor connections.
    /// Verify that it can be:
    /// * returned in a list of endpoints
    /// * retrieve the connection id for that endpoint
    #[test]
    fn test_service_connected() {
        let mut transport = InprocTransport::default();
        let mut listener = transport.listen("inproc://test_service_connected").unwrap();

        let mesh = Mesh::new(512, 128);
        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(NoopAuthorizer::new("service-id")))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(Box::new(transport.clone()))
            .start()
            .expect("Unable to start Connection Manager");

        let connector = cm.connector();

        let (conn_tx, conn_rx) = mpsc::channel();

        let jh = thread::spawn(move || {
            let _connection = transport
                .connect("inproc://test_service_connected")
                .unwrap();

            // block until done
            conn_rx.recv().unwrap();
        });

        let service_conn_mgr = ServiceConnectionManagerBuilder::new()
            .with_connector(connector.clone())
            .start()
            .expect("Unable to start service manager");

        let service_connector = service_conn_mgr.service_connector();
        let (subs_tx, subs_rx) = mpsc::channel();
        service_connector
            .subscribe(subs_tx)
            .expect("Unable to get subscriber");

        let connection = listener.accept().unwrap();
        connector
            .add_inbound_connection(connection)
            .expect("Unable to add inbound connection");

        // wait to receive the notification
        let notification: ServiceConnectionNotification = subs_rx.recv().unwrap();
        match notification {
            ServiceConnectionNotification::Connected {
                endpoint,
                service_id,
            } => {
                assert_eq!(endpoint, "inproc://test_service_connected".to_string());
                assert_eq!(service_id, "service-id".to_string());
            }
            _ => panic!("Unexpected notification: {:?}", notification),
        }

        let service_connections = service_connector
            .list_service_connections()
            .expect("Unable to list service_connections");

        assert_eq!(vec!["service-id"], service_connections);
        let connection_id = service_connector
            .get_connection_id("service-id")
            .expect("Unable to get the connection_id");

        assert!(connection_id.is_some());

        let service_identity = service_connector
            .get_identity(connection_id.as_ref().unwrap())
            .expect("Unable to get the identity");

        assert_eq!("service-id", &service_identity.unwrap());

        // signal to drop the connection
        conn_tx.send(()).unwrap();
        jh.join().unwrap();

        service_conn_mgr.shutdown_and_wait();
        cm.shutdown_signaler().shutdown();
        cm.await_shutdown();
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
