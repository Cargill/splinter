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

use std::sync::mpsc::{channel, Sender};
use std::thread;

use protobuf::Message;

use crate::protos::network::{NetworkHeartbeat, NetworkMessage, NetworkMessageType};
use crate::threading::pacemaker;
use crate::transport::matrix::{ConnectionMatrixLifeCycle, ConnectionMatrixSender};
use crate::transport::Transport;

use super::error::ConnectionManagerError;
use super::{
    AuthResult, Authorizer, CmMessage, CmRequest, ConnectionManager, ConnectionManagerNotification,
    ConnectionManagerState, ConnectionMetadataExt, SubscriberMap,
};

const DEFAULT_HEARTBEAT_INTERVAL: u64 = 10;
const DEFAULT_MAXIMUM_RETRY_FREQUENCY: u64 = 300;

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
