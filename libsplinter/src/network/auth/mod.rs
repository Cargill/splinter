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

mod connection_manager;
mod handlers;
mod pool;

use std::collections::HashMap;
use std::fmt;
use std::sync::{mpsc, Arc, Mutex};

use protobuf::Message;

use crate::protocol::authorization::{AuthorizationMessage, ConnectRequest};
use crate::protos::authorization;
use crate::protos::network::{NetworkMessage, NetworkMessageType};
use crate::protos::prelude::*;
use crate::transport::{Connection, RecvError};

use self::handlers::create_authorization_dispatcher;
use self::pool::{ThreadPool, ThreadPoolBuilder};

const AUTHORIZATION_THREAD_POOL_SIZE: usize = 8;

/// The states of a connection during authorization.
#[derive(PartialEq, Debug, Clone)]
pub(crate) enum AuthorizationState {
    Unknown,
    Connecting,
    RemoteIdentified(String),
    RemoteAccepted,
    Authorized(String),
    Unauthorized,
}

impl fmt::Display for AuthorizationState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            AuthorizationState::Unknown => "Unknown",
            AuthorizationState::Connecting => "Connecting",
            AuthorizationState::RemoteIdentified(_) => "Remote Identified",
            AuthorizationState::RemoteAccepted => "Remote Accepted",
            AuthorizationState::Authorized(_) => "Authorized",
            AuthorizationState::Unauthorized => "Unauthorized",
        })
    }
}

type Identity = String;

/// The state transitions that can be applied on a connection during authorization.
#[derive(PartialEq, Debug)]
pub(crate) enum AuthorizationAction {
    Connecting,
    TrustIdentifying(Identity),
    Unauthorizing,
    RemoteAuthorizing,
}

impl fmt::Display for AuthorizationAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AuthorizationAction::Connecting => f.write_str("Connecting"),
            AuthorizationAction::TrustIdentifying(_) => f.write_str("TrustIdentifying"),
            AuthorizationAction::Unauthorizing => f.write_str("Unauthorizing"),
            AuthorizationAction::RemoteAuthorizing => f.write_str("RemoteAuthorizing"),
        }
    }
}

/// The errors that may occur for a connection during authorization.
#[derive(PartialEq, Debug)]
pub(crate) enum AuthorizationActionError {
    AlreadyConnecting,
    InvalidMessageOrder(AuthorizationState, AuthorizationAction),
    InternalError(String),
}

impl fmt::Display for AuthorizationActionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AuthorizationActionError::AlreadyConnecting => {
                f.write_str("Already attempting to connect.")
            }
            AuthorizationActionError::InvalidMessageOrder(start, action) => {
                write!(f, "Attempting to transition from {} via {}.", start, action)
            }
            AuthorizationActionError::InternalError(msg) => f.write_str(&msg),
        }
    }
}

#[derive(Debug)]
pub struct AuthorizationManagerError(pub String);

impl std::error::Error for AuthorizationManagerError {}

impl fmt::Display for AuthorizationManagerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Manages authorization states for connections on a network.
pub struct AuthorizationManager {
    local_identity: String,
    thread_pool: ThreadPool,
    shared: Arc<Mutex<ManagedAuthorizations>>,
}

impl AuthorizationManager {
    /// Constructs an AuthorizationManager
    pub fn new(local_identity: String) -> Result<Self, AuthorizationManagerError> {
        let thread_pool = ThreadPoolBuilder::new()
            .with_size(AUTHORIZATION_THREAD_POOL_SIZE)
            .with_prefix("AuthorizationManager-".into())
            .build()
            .map_err(|err| AuthorizationManagerError(err.to_string()))?;

        let shared = Arc::new(Mutex::new(ManagedAuthorizations::new()));

        Ok(Self {
            thread_pool,
            shared,
            local_identity,
        })
    }

    pub fn shutdown_signaler(&self) -> ShutdownSignaler {
        ShutdownSignaler {
            thread_pool_signaler: self.thread_pool.shutdown_signaler(),
        }
    }

    pub fn wait_for_shutdown(self) {
        self.thread_pool.join_all()
    }

    pub fn authorization_connector(&self) -> AuthorizationConnector {
        AuthorizationConnector {
            local_identity: self.local_identity.clone(),
            shared: Arc::clone(&self.shared),
            executor: self.thread_pool.executor(),
        }
    }
}

pub struct ShutdownSignaler {
    thread_pool_signaler: pool::ShutdownSignaler,
}

impl ShutdownSignaler {
    pub fn shutdown(&self) {
        self.thread_pool_signaler.shutdown();
    }
}

type Callback =
    Box<dyn Fn(ConnectionAuthorizationState) -> Result<(), Box<dyn std::error::Error>> + Send>;

pub struct AuthorizationConnector {
    local_identity: String,
    shared: Arc<Mutex<ManagedAuthorizations>>,
    executor: pool::JobExecutor,
}

impl AuthorizationConnector {
    pub fn add_connection(
        &self,
        connection_id: String,
        connection: Box<dyn Connection>,
        on_complete_callback: Callback,
    ) -> Result<(), AuthorizationManagerError> {
        let mut connection = connection;

        let (tx, rx) = mpsc::channel();
        let connection_shared = Arc::clone(&self.shared);
        let state_machine = AuthorizationManagerStateMachine {
            shared: Arc::clone(&self.shared),
        };
        let msg_sender = AuthorizationMessageSender { sender: tx };
        let dispatcher =
            create_authorization_dispatcher(self.local_identity.clone(), state_machine, msg_sender);
        self.executor.execute(move || {
            let connect_request_bytes = match connect_msg_bytes() {
                Ok(bytes) => bytes,
                Err(err) => {
                    error!(
                        "Unable to create connect request for {}; aborting auth: {}",
                        &connection_id, err
                    );
                    return;
                }
            };
            if let Err(err) = connection.send(&connect_request_bytes) {
                error!(
                    "Unable to send connect request to {}; aborting auth: {}",
                    &connection_id, err
                );
                return;
            }

            let authed_identity = 'main: loop {
                match connection.recv() {
                    Ok(bytes) => {
                        let mut msg: NetworkMessage = match protobuf::parse_from_bytes(&bytes) {
                            Ok(msg) => msg,
                            Err(err) => {
                                warn!("Received invalid network message: {}", err);
                                continue;
                            }
                        };

                        let message_type = msg.get_message_type();
                        if let Err(err) = dispatcher.dispatch(
                            connection_id.clone().into(),
                            &message_type,
                            msg.take_payload(),
                        ) {
                            error!(
                                "Unable to dispatch message of type {:?}: {}",
                                message_type, err
                            );
                        }
                    }
                    Err(RecvError::Disconnected) => {
                        error!("Connection unexpectedly disconnected; aborting authorization");
                        break 'main None;
                    }
                    Err(RecvError::IoError(err)) => {
                        error!("Unable to authorize connection due to I/O error: {}", err);
                        break 'main None;
                    }
                    Err(RecvError::ProtocolError(msg)) => {
                        error!(
                            "Unable to authorize connection due to protocol error: {}",
                            msg
                        );
                        break 'main None;
                    }
                    Err(RecvError::WouldBlock) => continue,
                }

                while let Ok(outgoing) = rx.try_recv() {
                    match connection.send(&outgoing) {
                        Ok(()) => (),
                        Err(err) => {
                            error!("Unable to send outgoing message; aborting auth: {}", err);
                            break 'main None;
                        }
                    }
                }

                let mut shared = match connection_shared.lock() {
                    Ok(shared) => shared,
                    Err(_) => {
                        error!("connection authorization lock poisoned; aborting auth");
                        break 'main None;
                    }
                };

                if let Some(true) = shared.is_complete(&connection_id) {
                    break 'main shared.take_connection_identity(&connection_id);
                }
            };

            let auth_state = if let Some(identity) = authed_identity {
                ConnectionAuthorizationState::Authorized {
                    connection_id,
                    connection,
                    identity,
                }
            } else {
                ConnectionAuthorizationState::Unauthorized {
                    connection_id,
                    connection,
                }
            };

            if let Err(err) = on_complete_callback(auth_state) {
                error!("unable to pass auth result to callback: {}", err);
            }
        });

        Ok(())
    }
}

fn connect_msg_bytes() -> Result<Vec<u8>, AuthorizationManagerError> {
    let mut network_msg = NetworkMessage::new();
    network_msg.set_message_type(NetworkMessageType::AUTHORIZATION);

    let connect_msg = AuthorizationMessage::ConnectRequest(ConnectRequest::Bidirectional);
    network_msg.set_payload(
        IntoBytes::<authorization::AuthorizationMessage>::into_bytes(connect_msg).map_err(
            |err| AuthorizationManagerError(format!("Unable to send connect request: {}", err)),
        )?,
    );

    network_msg.write_to_bytes().map_err(|err| {
        AuthorizationManagerError(format!("Unable to send connect request: {}", err))
    })
}

#[derive(Clone)]
pub struct AuthorizationMessageSender {
    sender: mpsc::Sender<Vec<u8>>,
}

impl AuthorizationMessageSender {
    pub fn send(&self, msg: Vec<u8>) -> Result<(), Vec<u8>> {
        self.sender.send(msg).map_err(|err| err.0)
    }
}

#[derive(Clone, Default)]
pub struct AuthorizationManagerStateMachine {
    shared: Arc<Mutex<ManagedAuthorizations>>,
}

impl AuthorizationManagerStateMachine {
    /// Transitions from one authorization state to another
    ///
    /// Errors
    ///
    /// The errors are error messages that should be returned on the appropriate message
    pub(crate) fn next_state(
        &self,
        connection_id: &str,
        action: AuthorizationAction,
    ) -> Result<AuthorizationState, AuthorizationActionError> {
        let mut shared = self.shared.lock().map_err(|_| {
            AuthorizationActionError::InternalError("Authorization pool lock was poisoned".into())
        })?;

        let cur_state = shared
            .states
            .entry(connection_id.to_string())
            .or_insert(AuthorizationState::Unknown);

        if action == AuthorizationAction::Unauthorizing {
            *cur_state = AuthorizationState::Unauthorized;
            return Ok(AuthorizationState::Unauthorized);
        }

        match &*cur_state {
            AuthorizationState::Unknown => match action {
                AuthorizationAction::Connecting => {
                    *cur_state = AuthorizationState::Connecting;
                    Ok(AuthorizationState::Connecting)
                }
                _ => Err(AuthorizationActionError::InvalidMessageOrder(
                    AuthorizationState::Unknown,
                    action,
                )),
            },
            AuthorizationState::Connecting => match action {
                AuthorizationAction::Connecting => Err(AuthorizationActionError::AlreadyConnecting),
                AuthorizationAction::TrustIdentifying(identity) => {
                    let new_state = AuthorizationState::RemoteIdentified(identity);
                    *cur_state = new_state.clone();
                    // Verify pub key allowed
                    Ok(new_state)
                }
                AuthorizationAction::RemoteAuthorizing => {
                    *cur_state = AuthorizationState::RemoteAccepted;
                    Ok(AuthorizationState::RemoteAccepted)
                }
                _ => Err(AuthorizationActionError::InvalidMessageOrder(
                    AuthorizationState::Connecting,
                    action,
                )),
            },
            AuthorizationState::RemoteIdentified(identity) => match action {
                AuthorizationAction::RemoteAuthorizing => {
                    let new_state = AuthorizationState::Authorized(identity.clone());
                    *cur_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidMessageOrder(
                    AuthorizationState::RemoteIdentified(identity.clone()),
                    action,
                )),
            },
            AuthorizationState::RemoteAccepted => match action {
                AuthorizationAction::TrustIdentifying(identity) => {
                    let new_state = AuthorizationState::Authorized(identity);
                    *cur_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidMessageOrder(
                    AuthorizationState::RemoteAccepted,
                    action,
                )),
            },
            _ => Err(AuthorizationActionError::InvalidMessageOrder(
                cur_state.clone(),
                action,
            )),
        }
    }
}

#[derive(Default)]
struct ManagedAuthorizations {
    states: HashMap<String, AuthorizationState>,
}

impl ManagedAuthorizations {
    fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    fn take_connection_identity(&mut self, connection_id: &str) -> Option<String> {
        self.states
            .remove(connection_id)
            .and_then(|state| match state {
                AuthorizationState::Authorized(identity) => Some(identity),
                _ => None,
            })
    }

    fn is_complete(&self, connection_id: &str) -> Option<bool> {
        self.states.get(connection_id).map(|state|
            matches!(state, AuthorizationState::Authorized(_) | AuthorizationState::Unauthorized))
    }
}

pub enum ConnectionAuthorizationState {
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

impl std::fmt::Debug for ConnectionAuthorizationState {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ConnectionAuthorizationState::Authorized {
                connection_id,
                identity,
                ..
            } => f
                .debug_struct("Authorized")
                .field("connection_id", connection_id)
                .field("identity", identity)
                .finish(),
            ConnectionAuthorizationState::Unauthorized { connection_id, .. } => f
                .debug_struct("Unauthorized")
                .field("connection_id", connection_id)
                .finish(),
        }
    }
}

#[cfg(test)]
pub(in crate::network) mod tests {
    use super::*;

    use protobuf::Message;

    use crate::mesh::{Envelope, Mesh};
    use crate::protocol::authorization::{
        AuthorizationMessage, AuthorizationType, Authorized, ConnectRequest, ConnectResponse,
        TrustRequest,
    };
    use crate::protos::authorization;
    use crate::protos::network::{NetworkMessage, NetworkMessageType};

    impl AuthorizationManager {
        /// A test friendly shutdown and wait method.
        pub fn shutdown_and_await(self) {
            self.shutdown_signaler().shutdown();
            self.wait_for_shutdown();
        }
    }

    pub(in crate::network) fn negotiation_connection_auth(
        mesh: &Mesh,
        connection_id: &str,
        expected_identity: &str,
    ) {
        let env = mesh.recv().expect("unable to receive from mesh");

        // receive the connect request from the connection manager
        assert_eq!(connection_id, env.id());
        let connect_request = read_auth_message(env.payload());
        assert!(matches!(
            connect_request,
            AuthorizationMessage::ConnectRequest(ConnectRequest::Bidirectional)
        ));

        // send our own connect request
        let env = write_auth_message(
            connection_id,
            AuthorizationMessage::ConnectRequest(ConnectRequest::Unidirectional),
        );
        mesh.send(env).expect("Unable to send connect response");

        let env = write_auth_message(
            connection_id,
            AuthorizationMessage::ConnectResponse(ConnectResponse {
                accepted_authorization_types: vec![AuthorizationType::Trust],
            }),
        );
        mesh.send(env).expect("Unable to send connect response");

        // receive the connect response
        let env = mesh.recv().expect("unable to receive from mesh");
        assert_eq!(connection_id, env.id());
        let connect_response = read_auth_message(env.payload());
        assert!(matches!(
            connect_response,
            AuthorizationMessage::ConnectResponse(_)
        ));

        // receive the trust request
        let env = mesh.recv().expect("unable to receive from mesh");
        assert_eq!(connection_id, env.id());
        let trust_request = read_auth_message(env.payload());
        assert!(matches!(
            trust_request,
            AuthorizationMessage::TrustRequest(TrustRequest { .. })
        ));

        // send authorized
        let env = write_auth_message(connection_id, AuthorizationMessage::Authorized(Authorized));
        mesh.send(env).expect("unable to send authorized");

        // send trust request
        let env = write_auth_message(
            connection_id,
            AuthorizationMessage::TrustRequest(TrustRequest {
                identity: expected_identity.to_string(),
            }),
        );
        mesh.send(env).expect("unable to send authorized");

        // receive authorized
        let env = mesh.recv().expect("unable to receive from mesh");
        assert_eq!(connection_id, env.id());
        let trust_request = read_auth_message(env.payload());
        assert!(matches!(trust_request, AuthorizationMessage::Authorized(_)));
    }

    fn read_auth_message(bytes: &[u8]) -> AuthorizationMessage {
        let msg: NetworkMessage =
            protobuf::parse_from_bytes(bytes).expect("Cannot parse network message");

        assert_eq!(NetworkMessageType::AUTHORIZATION, msg.get_message_type());

        FromBytes::<authorization::AuthorizationMessage>::from_bytes(msg.get_payload())
            .expect("Unable to parse bytes")
    }

    fn write_auth_message(connection_id: &str, auth_msg: AuthorizationMessage) -> Envelope {
        let mut msg = NetworkMessage::new();
        msg.set_message_type(NetworkMessageType::AUTHORIZATION);
        msg.set_payload(
            IntoBytes::<authorization::AuthorizationMessage>::into_bytes(auth_msg)
                .expect("Unable to convert into bytes"),
        );

        Envelope::new(
            connection_id.to_string(),
            msg.write_to_bytes().expect("Unable to write to bytes"),
        )
    }
}
