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

/// The states of a connection during authorization.
#[derive(PartialEq, Debug, Clone)]
pub(crate) enum AuthorizationState {
    Unknown,
    Connecting,
    Authorized,
    Unauthorized,
}

impl fmt::Display for AuthorizationState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            AuthorizationState::Unknown => "Unknown",
            AuthorizationState::Connecting => "Connecting",
            AuthorizationState::Authorized => "Authorized",
            AuthorizationState::Unauthorized => "Unauthorized",
        })
    }
}

type Identity = String;

/// The state transitions that can be applied on an connection during authorization.
#[derive(PartialEq, Debug)]
pub(crate) enum AuthorizationAction {
    Connecting,
    TrustIdentifying(Identity),
    Unauthorizing,
}

impl fmt::Display for AuthorizationAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AuthorizationAction::Connecting => f.write_str("Connecting"),
            AuthorizationAction::TrustIdentifying(_) => f.write_str("TrustIdentifying"),
            AuthorizationAction::Unauthorizing => f.write_str("Unauthorizing"),
        }
    }
}

/// The errors that may occur for a connection during authorization.
#[derive(PartialEq, Debug)]
pub(crate) enum AuthorizationActionError {
    AlreadyConnecting,
    InvalidMessageOrder(AuthorizationState, AuthorizationAction),
    SystemFailure(String),
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
            AuthorizationActionError::SystemFailure(msg) => f.write_str(&msg),
        }
    }
}

#[derive(Debug)]
pub struct AuthorizationPoolError(pub String);

impl std::error::Error for AuthorizationPoolError {}

impl fmt::Display for AuthorizationPoolError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Manages authorization states for connections on a network.
pub struct AuthorizationPool {
    local_identity: String,
    thread_pool: ThreadPool,
    shared: Arc<Mutex<ManagedAuthorizations>>,
}

impl AuthorizationPool {
    /// Constructs an AuthorizationManager
    pub fn new(local_identity: String) -> Result<Self, AuthorizationPoolError> {
        let thread_pool = ThreadPoolBuilder::new()
            .with_size(8)
            .with_prefix("AuthorizationPool-".into())
            .build()
            .map_err(|err| AuthorizationPoolError(err.to_string()))?;

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

    pub fn pool_authorizer(&self) -> PoolAuthorizer {
        PoolAuthorizer {
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

pub struct PoolAuthorizer {
    local_identity: String,
    shared: Arc<Mutex<ManagedAuthorizations>>,
    executor: pool::JobExecutor,
}

impl PoolAuthorizer {
    pub fn add_connection(
        &self,
        connection_id: String,
        connection: Box<dyn Connection>,
        on_complete_callback: Callback,
    ) -> Result<(), AuthorizationPoolError> {
        let mut connection = connection;

        let (tx, rx) = mpsc::channel();
        let connection_shared = Arc::clone(&self.shared);
        let state_machine = AuthorizationPoolStateMachine {
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

                if shared.is_complete(&connection_id).is_some() {
                    break 'main shared.cleanup_connection_state(&connection_id);
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

fn connect_msg_bytes() -> Result<Vec<u8>, AuthorizationPoolError> {
    let mut network_msg = NetworkMessage::new();
    network_msg.set_message_type(NetworkMessageType::AUTHORIZATION);

    let connect_msg = AuthorizationMessage::ConnectRequest(ConnectRequest::Bidirectional);
    network_msg.set_payload(
        IntoBytes::<authorization::AuthorizationMessage>::into_bytes(connect_msg).map_err(
            |err| AuthorizationPoolError(format!("Unable to send connect request: {}", err)),
        )?,
    );

    network_msg
        .write_to_bytes()
        .map_err(|err| AuthorizationPoolError(format!("Unable to send connect request: {}", err)))
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
pub struct AuthorizationPoolStateMachine {
    shared: Arc<Mutex<ManagedAuthorizations>>,
}

impl AuthorizationPoolStateMachine {
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
            AuthorizationActionError::SystemFailure("Authorization pool lock was poisoned".into())
        })?;

        let cur_state = shared
            .states
            .get(connection_id)
            .unwrap_or(&AuthorizationState::Unknown);
        match *cur_state {
            AuthorizationState::Unknown => match action {
                AuthorizationAction::Connecting => {
                    // Here the decision for Challenges will be made.
                    shared
                        .states
                        .insert(connection_id.to_string(), AuthorizationState::Connecting);
                    Ok(AuthorizationState::Connecting)
                }
                AuthorizationAction::Unauthorizing => {
                    shared.mark_complete(connection_id, None);
                    Ok(AuthorizationState::Unauthorized)
                }
                _ => Err(AuthorizationActionError::InvalidMessageOrder(
                    AuthorizationState::Unknown,
                    action,
                )),
            },
            AuthorizationState::Connecting => match action {
                AuthorizationAction::Connecting => Err(AuthorizationActionError::AlreadyConnecting),
                AuthorizationAction::TrustIdentifying(identity) => {
                    // Verify pub key allowed
                    shared.mark_complete(connection_id, Some(identity));
                    Ok(AuthorizationState::Authorized)
                }
                AuthorizationAction::Unauthorizing => {
                    shared.mark_complete(connection_id, None);

                    Ok(AuthorizationState::Unauthorized)
                }
            },
            AuthorizationState::Authorized => match action {
                AuthorizationAction::Unauthorizing => {
                    shared.mark_complete(connection_id, None);

                    Ok(AuthorizationState::Unauthorized)
                }
                _ => Err(AuthorizationActionError::InvalidMessageOrder(
                    AuthorizationState::Authorized,
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

type Callback =
    Box<dyn Fn(ConnectionAuthorizationState) -> Result<(), Box<dyn std::error::Error>> + Send>;

#[derive(Default)]
struct ManagedAuthorizations {
    states: HashMap<String, AuthorizationState>,
    complete_and_authorized: HashMap<String, Option<String>>,
}

impl ManagedAuthorizations {
    fn new() -> Self {
        Self {
            states: HashMap::new(),
            complete_and_authorized: HashMap::new(),
        }
    }

    fn cleanup_connection_state(&mut self, connection_id: &str) -> Option<String> {
        self.states.remove(connection_id);
        self.complete_and_authorized.remove(connection_id).flatten()
    }

    // Mark complete with an optional identity
    fn mark_complete(&mut self, connection_id: &str, authorized_identity: Option<String>) {
        self.complete_and_authorized
            .insert(connection_id.to_string(), authorized_identity);
    }

    fn is_complete(&self, connection_id: &str) -> Option<bool> {
        self.complete_and_authorized
            .get(connection_id)
            .map(|ident| ident.is_some())
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

    impl AuthorizationPool {
        /// A test friendly shutdown and wait method.
        pub fn shutdown_and_await(self) {
            self.shutdown_signaler().shutdown();
            self.wait_for_shutdown();
        }
    }

    pub(in crate::network) fn negotiation_connection_auth(mesh: &Mesh, connection_id: &str) {
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
                identity: "test_identity".to_string(),
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
