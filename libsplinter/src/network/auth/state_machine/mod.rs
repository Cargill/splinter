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
pub mod trust_v0;
#[cfg(feature = "trust-authorization")]
pub mod trust_v1;

use std::fmt;
use std::sync::{Arc, Mutex};

use self::trust_v0::{TrustV0AuthorizationAction, TrustV0AuthorizationState};
#[cfg(feature = "trust-authorization")]
use self::trust_v1::{TrustAuthorizationAction, TrustAuthorizationState};

use super::{ManagedAuthorizationState, ManagedAuthorizations};

#[derive(Debug, PartialEq, Clone)]
pub enum Identity {
    Trust { identity: String },
}

#[derive(PartialEq, Debug, Clone)]
pub(crate) enum AuthorizationState {
    Unknown,
    AuthComplete(Option<Identity>),
    Unauthorized,

    #[cfg(feature = "trust-authorization")]
    ProtocolAgreeing,
    TrustV0(TrustV0AuthorizationState),
    #[cfg(feature = "trust-authorization")]
    Trust(TrustAuthorizationState),
}

impl fmt::Display for AuthorizationState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AuthorizationState::Unknown => f.write_str("Unknown"),
            #[cfg(feature = "trust-authorization")]
            AuthorizationState::ProtocolAgreeing => f.write_str("ProtocolAgreeing"),
            AuthorizationState::TrustV0(action) => write!(f, "TrustV0: {}", action),
            #[cfg(feature = "trust-authorization")]
            AuthorizationState::Trust(action) => write!(f, "Trust: {}", action),

            AuthorizationState::AuthComplete(_) => f.write_str("Authorization Complete"),
            AuthorizationState::Unauthorized => f.write_str("Unauthorized"),
        }
    }
}

/// The state transitions that can be applied on a connection during authorization.
#[derive(PartialEq, Debug)]
pub(crate) enum AuthorizationAction {
    Connecting,
    #[cfg(feature = "trust-authorization")]
    ProtocolAgreeing,
    TrustV0(TrustV0AuthorizationAction),
    #[cfg(feature = "trust-authorization")]
    Trust(TrustAuthorizationAction),

    Unauthorizing,
}

impl fmt::Display for AuthorizationAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AuthorizationAction::Connecting => f.write_str("Connecting"),
            AuthorizationAction::Unauthorizing => f.write_str("Unauthorizing"),
            #[cfg(feature = "trust-authorization")]
            AuthorizationAction::ProtocolAgreeing => f.write_str("ProtocolAgreeing"),
            AuthorizationAction::TrustV0(_) => f.write_str("TrustV0"),
            #[cfg(feature = "trust-authorization")]
            AuthorizationAction::Trust(_) => f.write_str("Trust"),
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
                f.write_str("Already attempting to connect")
            }
            AuthorizationActionError::InvalidMessageOrder(start, action) => {
                write!(f, "Attempting to transition from {} via {}", start, action)
            }
            AuthorizationActionError::InternalError(msg) => f.write_str(&msg),
        }
    }
}

#[derive(Clone, Default)]
pub struct AuthorizationManagerStateMachine {
    pub shared: Arc<Mutex<ManagedAuthorizations>>,
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

        let mut cur_state =
            shared
                .states
                .entry(connection_id.to_string())
                .or_insert(ManagedAuthorizationState {
                    local_state: AuthorizationState::Unknown,
                    remote_state: AuthorizationState::Unknown,
                });

        if action == AuthorizationAction::Unauthorizing {
            cur_state.local_state = AuthorizationState::Unauthorized;
            cur_state.remote_state = AuthorizationState::Unauthorized;
            return Ok(AuthorizationState::Unauthorized);
        }

        match cur_state.local_state.clone() {
            AuthorizationState::Unknown => match action {
                AuthorizationAction::Connecting => {
                    if cur_state.local_state
                        == AuthorizationState::TrustV0(TrustV0AuthorizationState::Connecting)
                    {
                        return Err(AuthorizationActionError::AlreadyConnecting);
                    };
                    cur_state.local_state =
                        AuthorizationState::TrustV0(TrustV0AuthorizationState::Connecting);
                    cur_state.remote_state = AuthorizationState::AuthComplete(None);
                    Ok(AuthorizationState::TrustV0(
                        TrustV0AuthorizationState::Connecting,
                    ))
                }
                #[cfg(feature = "trust-authorization")]
                AuthorizationAction::ProtocolAgreeing => {
                    cur_state.local_state = AuthorizationState::ProtocolAgreeing;
                    Ok(AuthorizationState::ProtocolAgreeing)
                }
                _ => Err(AuthorizationActionError::InvalidMessageOrder(
                    AuthorizationState::Unknown,
                    action,
                )),
            },
            AuthorizationState::TrustV0(state) => match action {
                AuthorizationAction::TrustV0(action) => {
                    let new_state = state.next_state(action)?;
                    cur_state.local_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidMessageOrder(
                    AuthorizationState::TrustV0(state),
                    action,
                )),
            },
            // v1 state transitions
            #[cfg(feature = "trust-authorization")]
            AuthorizationState::ProtocolAgreeing => match action {
                AuthorizationAction::Trust(action) => {
                    let new_state =
                        TrustAuthorizationState::TrustConnecting.next_state(action, cur_state)?;
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidMessageOrder(
                    AuthorizationState::ProtocolAgreeing,
                    action,
                )),
            },
            #[cfg(feature = "trust-authorization")]
            AuthorizationState::Trust(state) => match action {
                AuthorizationAction::Trust(action) => {
                    let new_state = state.next_state(action, cur_state)?;
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidMessageOrder(
                    AuthorizationState::Trust(state),
                    action,
                )),
            },
            _ => Err(AuthorizationActionError::InvalidMessageOrder(
                cur_state.local_state.clone(),
                action,
            )),
        }
    }

    /// Transitions from one authorization state to another. This is specific to the remote node
    ///
    /// Errors
    ///
    /// The errors are error messages that should be returned on the appropriate message
    #[cfg(feature = "trust-authorization")]
    pub(crate) fn next_remote_state(
        &self,
        connection_id: &str,
        action: AuthorizationAction,
    ) -> Result<AuthorizationState, AuthorizationActionError> {
        let mut shared = self.shared.lock().map_err(|_| {
            AuthorizationActionError::InternalError("Authorization pool lock was poisoned".into())
        })?;

        let mut cur_state =
            shared
                .states
                .entry(connection_id.to_string())
                .or_insert(ManagedAuthorizationState {
                    local_state: AuthorizationState::Unknown,
                    remote_state: AuthorizationState::Unknown,
                });

        if action == AuthorizationAction::Unauthorizing {
            cur_state.local_state = AuthorizationState::Unauthorized;
            cur_state.remote_state = AuthorizationState::Unauthorized;
            return Ok(AuthorizationState::Unauthorized);
        }

        match cur_state.remote_state.clone() {
            AuthorizationState::Unknown => match action {
                AuthorizationAction::ProtocolAgreeing => {
                    cur_state.remote_state = AuthorizationState::ProtocolAgreeing;
                    Ok(AuthorizationState::ProtocolAgreeing)
                }
                _ => Err(AuthorizationActionError::InvalidMessageOrder(
                    AuthorizationState::Unknown,
                    action,
                )),
            },
            // v1 state transitions
            AuthorizationState::ProtocolAgreeing => match action {
                AuthorizationAction::Trust(action) => {
                    let new_state = TrustAuthorizationState::TrustConnecting
                        .next_remote_state(action, cur_state)?;
                    cur_state.remote_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidMessageOrder(
                    AuthorizationState::ProtocolAgreeing,
                    action,
                )),
            },
            #[cfg(feature = "trust-authorization")]
            AuthorizationState::Trust(state) => match action {
                AuthorizationAction::Trust(action) => {
                    let new_state = state.next_remote_state(action, cur_state)?;
                    cur_state.remote_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidMessageOrder(
                    AuthorizationState::Trust(state),
                    action,
                )),
            },
            _ => Err(AuthorizationActionError::InvalidMessageOrder(
                cur_state.remote_state.clone(),
                action,
            )),
        }
    }
}
