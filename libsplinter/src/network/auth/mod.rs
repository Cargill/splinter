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

mod connection_manager;
mod handlers;
mod pool;
mod state_machine;

use std::collections::HashMap;
use std::fmt;
use std::sync::{mpsc, Arc, Mutex};

#[cfg(feature = "challenge-authorization")]
use cylinder::{Signer, VerifierFactory};
use protobuf::Message;

#[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
use crate::protocol::authorization::AuthProtocolRequest;
use crate::protocol::authorization::AuthorizationMessage;
#[cfg(not(all(feature = "trust-authorization", feature = "challenge-authorization")))]
use crate::protocol::authorization::ConnectRequest;
use crate::protocol::network::NetworkMessage;
#[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
use crate::protocol::{PEER_AUTHORIZATION_PROTOCOL_MIN, PEER_AUTHORIZATION_PROTOCOL_VERSION};
use crate::protos::network;
use crate::protos::prelude::*;
use crate::transport::{Connection, RecvError};

use self::handlers::create_authorization_dispatcher;
use self::pool::{ThreadPool, ThreadPoolBuilder};
#[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
pub(crate) use self::state_machine::AuthorizationLocalAction;
pub(crate) use self::state_machine::{
    AuthorizationActionError, AuthorizationLocalState, AuthorizationManagerStateMachine,
    AuthorizationRemoteAction, AuthorizationRemoteState, Identity,
};

const AUTHORIZATION_THREAD_POOL_SIZE: usize = 8;

/// Used to track both the local nodes authorization state and the authorization state of the
/// remote node. For v1, authorization is happening in parallel so the states must be tracked
/// separately.
#[derive(Debug, Clone)]
pub(crate) struct ManagedAuthorizationState {
    // Local node state while authorizing with remote node
    local_state: AuthorizationLocalState,
    // Remote node state
    remote_state: AuthorizationRemoteState,

    // Tracks whether the local node has completed authorization with the remote node
    received_complete: bool,
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
    #[cfg(feature = "challenge-authorization")]
    signers: Vec<Box<dyn Signer>>,
    thread_pool: ThreadPool,
    shared: Arc<Mutex<ManagedAuthorizations>>,
    #[cfg(feature = "challenge-authorization")]
    verifier_factory: Arc<Mutex<Box<dyn VerifierFactory>>>,
}

impl AuthorizationManager {
    /// Constructs an AuthorizationManager
    pub fn new(
        local_identity: String,
        #[cfg(feature = "challenge-authorization")] signers: Vec<Box<dyn Signer>>,
        #[cfg(feature = "challenge-authorization")] verifier_factory: Arc<
            Mutex<Box<dyn VerifierFactory>>,
        >,
    ) -> Result<Self, AuthorizationManagerError> {
        let thread_pool = ThreadPoolBuilder::new()
            .with_size(AUTHORIZATION_THREAD_POOL_SIZE)
            .with_prefix("AuthorizationManager-".into())
            .build()
            .map_err(|err| AuthorizationManagerError(err.to_string()))?;

        let shared = Arc::new(Mutex::new(ManagedAuthorizations::new()));

        Ok(Self {
            local_identity,
            #[cfg(feature = "challenge-authorization")]
            signers,
            thread_pool,
            shared,
            #[cfg(feature = "challenge-authorization")]
            verifier_factory,
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
            #[cfg(feature = "challenge-authorization")]
            signers: self.signers.clone(),
            shared: Arc::clone(&self.shared),
            executor: self.thread_pool.executor(),
            #[cfg(feature = "challenge-authorization")]
            verifier_factory: self.verifier_factory.clone(),
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
    #[cfg(feature = "challenge-authorization")]
    signers: Vec<Box<dyn Signer>>,
    shared: Arc<Mutex<ManagedAuthorizations>>,
    executor: pool::JobExecutor,
    #[cfg(feature = "challenge-authorization")]
    verifier_factory: Arc<Mutex<Box<dyn VerifierFactory>>>,
}

impl AuthorizationConnector {
    pub fn add_connection(
        &self,
        connection_id: String,
        connection: Box<dyn Connection>,
        #[cfg(feature = "challenge-authorization")] expected_authorization: Option<
            ConnectionAuthorizationType,
        >,
        #[cfg(feature = "challenge-authorization")] local_authorization: Option<
            ConnectionAuthorizationType,
        >,
        on_complete_callback: Callback,
    ) -> Result<(), AuthorizationManagerError> {
        let mut connection = connection;

        let (tx, rx) = mpsc::channel();
        let connection_shared = Arc::clone(&self.shared);
        let state_machine = AuthorizationManagerStateMachine {
            shared: Arc::clone(&self.shared),
        };
        let msg_sender = AuthorizationMessageSender { sender: tx };
        #[cfg(feature = "challenge-authorization")]
        let verifier = self
            .verifier_factory
            .lock()
            .map_err(|_| AuthorizationManagerError("VerifierFactory lock poisoned".to_string()))?
            .new_verifier();

        #[cfg(feature = "challenge-authorization")]
        let nonce: Vec<u8> = (0..70).map(|_| rand::random::<u8>()).collect();
        let dispatcher = create_authorization_dispatcher(
            self.local_identity.clone(),
            #[cfg(feature = "challenge-authorization")]
            self.signers.clone(),
            // need to allow clone because it is required if trust authorization is enabled
            #[allow(clippy::redundant_clone)]
            state_machine.clone(),
            msg_sender,
            #[cfg(feature = "challenge-authorization")]
            nonce,
            #[cfg(feature = "challenge-authorization")]
            expected_authorization.clone(),
            #[cfg(feature = "challenge-authorization")]
            local_authorization.clone(),
            #[cfg(feature = "challenge-authorization")]
            verifier,
        )
        .map_err(|err| {
            AuthorizationManagerError(format!("Unable to setup authorization dispatcher: {}", err))
        })?;

        self.executor.execute(move || {
            #[cfg(not(all(feature = "trust-authorization", feature = "challenge-authorization")))]
            {
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
            }

            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
            {
                let protocol_request_bytes = match protocol_msg_bytes() {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        error!(
                            "Unable to create protocol request for {}; aborting auth: {}",
                            &connection_id, err
                        );
                        return;
                    }
                };
                if let Err(err) = connection.send(&protocol_request_bytes) {
                    error!(
                        "Unable to send protocol request to {}; aborting auth: {}",
                        &connection_id, err
                    );
                    return;
                }

                if state_machine
                    .next_local_state(
                        &connection_id,
                        AuthorizationLocalAction::SendAuthProtocolRequest,
                    )
                    .is_err()
                {
                    error!(
                        "Unable to update state from Start to WaitingForAuthProtocolResponse for {}",
                        &connection_id,
                    )
                };
            }

            let authed_identity = 'main: loop {
                match connection.recv() {
                    Ok(bytes) => {
                        let mut msg: network::NetworkMessage =
                            match Message::parse_from_bytes(&bytes) {
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

            let auth_state = if let Some(auth_identity) = authed_identity {
                match auth_identity {
                    Identity::Trust { identity } => ConnectionAuthorizationState::Authorized {
                        connection_id,
                        connection,
                        #[cfg(feature = "challenge-authorization")]
                        expected_authorization,
                        #[cfg(feature = "challenge-authorization")]
                        local_authorization,
                        identity: ConnectionAuthorizationType::Trust { identity },
                    },
                    #[cfg(feature = "challenge-authorization")]
                    Identity::Challenge { public_key } => {
                        ConnectionAuthorizationState::Authorized {
                            connection_id: connection_id.clone(),
                            connection,
                            identity: ConnectionAuthorizationType::Challenge { public_key },
                            expected_authorization,
                            local_authorization,
                        }
                    }
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

#[cfg(not(all(feature = "trust-authorization", feature = "challenge-authorization")))]
fn connect_msg_bytes() -> Result<Vec<u8>, AuthorizationManagerError> {
    let connect_msg = AuthorizationMessage::ConnectRequest(ConnectRequest::Bidirectional);

    IntoBytes::<network::NetworkMessage>::into_bytes(NetworkMessage::from(connect_msg)).map_err(
        |err| AuthorizationManagerError(format!("Unable to send connect request: {}", err)),
    )
}

#[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
fn protocol_msg_bytes() -> Result<Vec<u8>, AuthorizationManagerError> {
    let connect_msg = AuthorizationMessage::AuthProtocolRequest(AuthProtocolRequest {
        auth_protocol_min: PEER_AUTHORIZATION_PROTOCOL_MIN,
        auth_protocol_max: PEER_AUTHORIZATION_PROTOCOL_VERSION,
    });

    IntoBytes::<network::NetworkMessage>::into_bytes(NetworkMessage::from(connect_msg)).map_err(
        |err| AuthorizationManagerError(format!("Unable to send connect request: {}", err)),
    )
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

#[derive(Default)]
pub struct ManagedAuthorizations {
    states: HashMap<String, ManagedAuthorizationState>,
}

impl ManagedAuthorizations {
    fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    fn take_connection_identity(&mut self, connection_id: &str) -> Option<Identity> {
        self.states.remove(connection_id).and_then(|managed_state| {
            match managed_state.remote_state {
                AuthorizationRemoteState::Done(identity) => Some(identity),
                _ => None,
            }
        })
    }

    fn is_complete(&self, connection_id: &str) -> Option<bool> {
        self.states.get(connection_id).map(|managed_state| {
            matches!(
                (&managed_state.local_state, &managed_state.remote_state),
                (
                    AuthorizationLocalState::AuthorizedAndComplete,
                    AuthorizationRemoteState::Done(_),
                ) | (
                    AuthorizationLocalState::Unauthorized,
                    AuthorizationRemoteState::Unauthorized
                )
            )
        })
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum ConnectionAuthorizationType {
    Trust {
        identity: String,
    },
    #[cfg(feature = "challenge-authorization")]
    Challenge {
        public_key: Vec<u8>,
    },
}

pub enum ConnectionAuthorizationState {
    Authorized {
        connection_id: String,
        identity: ConnectionAuthorizationType,
        connection: Box<dyn Connection>,
        // information required if reconnect needs to be attempted
        #[cfg(feature = "challenge-authorization")]
        expected_authorization: Option<ConnectionAuthorizationType>,
        #[cfg(feature = "challenge-authorization")]
        local_authorization: Option<ConnectionAuthorizationType>,
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
    #[cfg(feature = "trust-authorization")]
    use crate::protocol::authorization::{
        AuthComplete, AuthProtocolRequest, AuthProtocolResponse, AuthTrustRequest,
        AuthTrustResponse, AuthorizationMessage, PeerAuthorizationType,
    };
    #[cfg(not(feature = "trust-authorization"))]
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

    #[cfg(not(feature = "trust-authorization"))]
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

    #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
    pub(in crate::network) fn negotiation_connection_auth(
        mesh: &Mesh,
        connection_id: &str,
        expected_identity: &str,
    ) {
        let env = mesh.recv().expect("unable to receive from mesh");

        // receive the protocol request from the connection manager
        assert_eq!(connection_id, env.id());
        let connect_request = read_auth_message(env.payload());
        assert!(matches!(
            connect_request,
            AuthorizationMessage::AuthProtocolRequest(_)
        ));

        // send our own protocol_request
        let env = write_auth_message(
            connection_id,
            AuthorizationMessage::AuthProtocolRequest(AuthProtocolRequest {
                auth_protocol_min: PEER_AUTHORIZATION_PROTOCOL_MIN,
                auth_protocol_max: PEER_AUTHORIZATION_PROTOCOL_VERSION,
            }),
        );
        mesh.send(env).expect("Unable to send protocol request");

        let env = write_auth_message(
            connection_id,
            AuthorizationMessage::AuthProtocolResponse(AuthProtocolResponse {
                auth_protocol: PEER_AUTHORIZATION_PROTOCOL_VERSION,
                accepted_authorization_type: vec![PeerAuthorizationType::Trust],
            }),
        );
        mesh.send(env).expect("Unable to send protocol request");

        // receive the protocol response
        let env = mesh.recv().expect("unable to receive from mesh");
        assert_eq!(connection_id, env.id());
        let protocol_response = read_auth_message(env.payload());
        assert!(matches!(
            protocol_response,
            AuthorizationMessage::AuthProtocolResponse(_)
        ));

        // receive the trust request
        let env = mesh.recv().expect("unable to receive from mesh");
        assert_eq!(connection_id, env.id());
        let trust_request = read_auth_message(env.payload());
        assert!(matches!(
            trust_request,
            AuthorizationMessage::AuthTrustRequest(AuthTrustRequest { .. })
        ));

        // send trust response
        let env = write_auth_message(
            connection_id,
            AuthorizationMessage::AuthTrustResponse(AuthTrustResponse),
        );
        mesh.send(env).expect("unable to send authorized");

        // receive authorized
        let env = mesh.recv().expect("unable to receive from mesh");
        assert_eq!(connection_id, env.id());
        let auth_complete = read_auth_message(env.payload());
        assert!(matches!(
            auth_complete,
            AuthorizationMessage::AuthComplete(_)
        ));

        // send trust request
        let env = write_auth_message(
            connection_id,
            AuthorizationMessage::AuthTrustRequest(AuthTrustRequest {
                identity: expected_identity.to_string(),
            }),
        );
        mesh.send(env).expect("unable to send authorized");

        // receive authorized
        let env = mesh.recv().expect("unable to receive from mesh");
        assert_eq!(connection_id, env.id());
        let trust_response = read_auth_message(env.payload());
        assert!(matches!(
            trust_response,
            AuthorizationMessage::AuthTrustResponse(_)
        ));

        // send auth complete
        let env = write_auth_message(
            connection_id,
            AuthorizationMessage::AuthComplete(AuthComplete),
        );
        mesh.send(env).expect("unable to send authorized");
    }

    fn read_auth_message(bytes: &[u8]) -> AuthorizationMessage {
        let msg: NetworkMessage =
            Message::parse_from_bytes(bytes).expect("Cannot parse network message");

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
