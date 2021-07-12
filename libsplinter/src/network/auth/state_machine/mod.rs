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
use self::trust_v1::{
    TrustAuthorizationLocalAction, TrustAuthorizationLocalState, TrustAuthorizationRemoteAction,
    TrustAuthorizationRemoteState,
};

use super::{ManagedAuthorizationState, ManagedAuthorizations};

#[derive(Debug, PartialEq, Clone)]
pub enum Identity {
    Trust { identity: String },
}

#[derive(PartialEq, Debug, Clone)]
pub(crate) enum AuthorizationRemoteState {
    Start,
    Done(Identity),
    Unauthorized,

    #[cfg(feature = "trust-authorization")]
    ReceivedAuthProtocolRequest,
    #[cfg(feature = "trust-authorization")]
    SentAuthProtocolResponse,

    TrustV0(TrustV0AuthorizationState),
    #[cfg(feature = "trust-authorization")]
    Trust(TrustAuthorizationRemoteState),
}

impl fmt::Display for AuthorizationRemoteState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AuthorizationRemoteState::Start => f.write_str("Start"),
            AuthorizationRemoteState::Done(_) => f.write_str("Done"),
            AuthorizationRemoteState::Unauthorized => f.write_str("Unauthorized"),

            #[cfg(feature = "trust-authorization")]
            AuthorizationRemoteState::ReceivedAuthProtocolRequest => {
                f.write_str("ReceivedAuthProtocolRequest")
            }
            #[cfg(feature = "trust-authorization")]
            AuthorizationRemoteState::SentAuthProtocolResponse => {
                f.write_str("SentAuthProtocolResponse")
            }
            AuthorizationRemoteState::TrustV0(state) => write!(f, "TrustV0: {}", state),
            #[cfg(feature = "trust-authorization")]
            AuthorizationRemoteState::Trust(state) => write!(f, "Trust: {}", state),
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub(crate) enum AuthorizationLocalState {
    Start,
    #[cfg(feature = "trust-authorization")]
    WaitingForAuthProtocolResponse,
    #[cfg(feature = "trust-authorization")]
    ReceivedAuthProtocolResponse,
    #[cfg(feature = "trust-authorization")]
    Authorized,
    #[cfg(feature = "trust-authorization")]
    WaitForComplete,
    AuthorizedAndComplete,
    Unauthorized,

    #[cfg(feature = "trust-authorization")]
    Trust(TrustAuthorizationLocalState),
}

impl fmt::Display for AuthorizationLocalState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AuthorizationLocalState::Start => f.write_str("Start"),
            #[cfg(feature = "trust-authorization")]
            AuthorizationLocalState::Authorized => f.write_str("Authorized"),
            AuthorizationLocalState::Unauthorized => f.write_str("Unauthorized"),
            #[cfg(feature = "trust-authorization")]
            AuthorizationLocalState::WaitingForAuthProtocolResponse => {
                f.write_str("WaitingForAuthProtocolResponse")
            }
            #[cfg(feature = "trust-authorization")]
            AuthorizationLocalState::ReceivedAuthProtocolResponse => {
                f.write_str("ReceivedAuthProtocolResponse")
            }
            #[cfg(feature = "trust-authorization")]
            AuthorizationLocalState::WaitForComplete => f.write_str("WaitForComplete"),
            AuthorizationLocalState::AuthorizedAndComplete => f.write_str("AuthorizedAndComplete"),

            #[cfg(feature = "trust-authorization")]
            AuthorizationLocalState::Trust(action) => write!(f, "Trust: {}", action),
        }
    }
}

/// The state transitions that can be applied on a connection during authorization.
#[derive(PartialEq, Debug)]
pub(crate) enum AuthorizationRemoteAction {
    Connecting,

    #[cfg(feature = "trust-authorization")]
    ReceiveAuthProtocolRequest,
    #[cfg(feature = "trust-authorization")]
    SendAuthProtocolResponse,

    TrustV0(TrustV0AuthorizationAction),
    #[cfg(feature = "trust-authorization")]
    Trust(TrustAuthorizationRemoteAction),

    Unauthorizing,
}

impl fmt::Display for AuthorizationRemoteAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AuthorizationRemoteAction::Connecting => f.write_str("Connecting"),
            #[cfg(feature = "trust-authorization")]
            AuthorizationRemoteAction::ReceiveAuthProtocolRequest => {
                f.write_str("ReceiveAuthProtocolRequest")
            }
            #[cfg(feature = "trust-authorization")]
            AuthorizationRemoteAction::SendAuthProtocolResponse => {
                f.write_str("SendAuthProtocolResponse")
            }
            AuthorizationRemoteAction::TrustV0(action) => write!(f, "TrusV0t: {}", action),
            #[cfg(feature = "trust-authorization")]
            AuthorizationRemoteAction::Trust(action) => write!(f, "Trust: {}", action),

            AuthorizationRemoteAction::Unauthorizing => f.write_str("Unauthorizing"),
        }
    }
}

/// The state transitions that can be applied on a connection during authorization.
#[derive(PartialEq, Debug)]
#[cfg(feature = "trust-authorization")]
pub(crate) enum AuthorizationLocalAction {
    SendAuthProtocolRequest,
    ReceiveAuthProtocolResponse,

    #[cfg(feature = "trust-authorization")]
    Trust(TrustAuthorizationLocalAction),

    SendAuthComplete,
    Unauthorizing,
}

#[cfg(feature = "trust-authorization")]
impl fmt::Display for AuthorizationLocalAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AuthorizationLocalAction::SendAuthProtocolRequest => {
                f.write_str("SendAuthProtocolRequest")
            }
            AuthorizationLocalAction::ReceiveAuthProtocolResponse => {
                f.write_str("SendAuthProtocolResponse")
            }

            #[cfg(feature = "trust-authorization")]
            AuthorizationLocalAction::Trust(action) => write!(f, "Trust: {}", action),

            AuthorizationLocalAction::SendAuthComplete => f.write_str("SendAuthComplete"),
            AuthorizationLocalAction::Unauthorizing => f.write_str("Unauthorizing"),
        }
    }
}

/// The errors that may occur for a connection during authorization.
#[derive(PartialEq, Debug)]
pub(crate) enum AuthorizationActionError {
    AlreadyConnecting,
    InvalidRemoteMessageOrder(AuthorizationRemoteState, AuthorizationRemoteAction),
    #[cfg(feature = "trust-authorization")]
    InvalidLocalMessageOrder(AuthorizationLocalState, AuthorizationLocalAction),
    InternalError(String),
}

impl fmt::Display for AuthorizationActionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AuthorizationActionError::AlreadyConnecting => {
                f.write_str("Already attempting to connect")
            }
            AuthorizationActionError::InvalidRemoteMessageOrder(start, action) => {
                write!(
                    f,
                    "Attempting to transition from remote state {} via {}",
                    start, action
                )
            }
            #[cfg(feature = "trust-authorization")]
            AuthorizationActionError::InvalidLocalMessageOrder(start, action) => {
                write!(
                    f,
                    "Attempting to transition from locale state {} via {}",
                    start, action
                )
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
    #[cfg(feature = "trust-authorization")]
    pub(crate) fn next_local_state(
        &self,
        connection_id: &str,
        action: AuthorizationLocalAction,
    ) -> Result<AuthorizationLocalState, AuthorizationActionError> {
        let mut shared = self.shared.lock().map_err(|_| {
            AuthorizationActionError::InternalError("Authorization pool lock was poisoned".into())
        })?;

        let mut cur_state =
            shared
                .states
                .entry(connection_id.to_string())
                .or_insert(ManagedAuthorizationState {
                    local_state: AuthorizationLocalState::Start,
                    remote_state: AuthorizationRemoteState::Start,
                    received_complete: false,
                });

        if action == AuthorizationLocalAction::Unauthorizing {
            cur_state.local_state = AuthorizationLocalState::Unauthorized;
            cur_state.remote_state = AuthorizationRemoteState::Unauthorized;
            return Ok(AuthorizationLocalState::Unauthorized);
        }

        match cur_state.local_state.clone() {
            AuthorizationLocalState::Start => match action {
                AuthorizationLocalAction::SendAuthProtocolRequest => {
                    cur_state.local_state = AuthorizationLocalState::WaitingForAuthProtocolResponse;
                    Ok(AuthorizationLocalState::WaitingForAuthProtocolResponse)
                }
                _ => Err(AuthorizationActionError::InvalidLocalMessageOrder(
                    AuthorizationLocalState::Start,
                    action,
                )),
            },
            // v1 state transitions
            AuthorizationLocalState::WaitingForAuthProtocolResponse => match action {
                AuthorizationLocalAction::ReceiveAuthProtocolResponse => {
                    cur_state.local_state = AuthorizationLocalState::ReceivedAuthProtocolResponse;
                    Ok(AuthorizationLocalState::ReceivedAuthProtocolResponse)
                }
                _ => Err(AuthorizationActionError::InvalidLocalMessageOrder(
                    AuthorizationLocalState::Start,
                    action,
                )),
            },
            AuthorizationLocalState::ReceivedAuthProtocolResponse => match action {
                #[cfg(feature = "trust-authorization")]
                AuthorizationLocalAction::Trust(action) => {
                    let new_state = TrustAuthorizationLocalState::TrustConnecting
                        .next_local_state(action, cur_state)?;
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidLocalMessageOrder(
                    AuthorizationLocalState::WaitingForAuthProtocolResponse,
                    action,
                )),
            },
            #[cfg(feature = "trust-authorization")]
            AuthorizationLocalState::Trust(state) => match action {
                AuthorizationLocalAction::Trust(action) => {
                    let new_state = state.next_local_state(action, cur_state)?;
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidLocalMessageOrder(
                    AuthorizationLocalState::Trust(state),
                    action,
                )),
            },
            AuthorizationLocalState::Authorized => match action {
                AuthorizationLocalAction::SendAuthComplete => {
                    let new_state = if cur_state.received_complete {
                        AuthorizationLocalState::AuthorizedAndComplete
                    } else {
                        AuthorizationLocalState::WaitForComplete
                    };

                    cur_state.local_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidLocalMessageOrder(
                    AuthorizationLocalState::Authorized,
                    action,
                )),
            },
            _ => Err(AuthorizationActionError::InvalidLocalMessageOrder(
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
    pub(crate) fn next_remote_state(
        &self,
        connection_id: &str,
        action: AuthorizationRemoteAction,
    ) -> Result<AuthorizationRemoteState, AuthorizationActionError> {
        let mut shared = self.shared.lock().map_err(|_| {
            AuthorizationActionError::InternalError("Authorization pool lock was poisoned".into())
        })?;

        let mut cur_state =
            shared
                .states
                .entry(connection_id.to_string())
                .or_insert(ManagedAuthorizationState {
                    local_state: AuthorizationLocalState::Start,
                    remote_state: AuthorizationRemoteState::Start,
                    received_complete: false,
                });

        if action == AuthorizationRemoteAction::Unauthorizing {
            cur_state.local_state = AuthorizationLocalState::Unauthorized;
            cur_state.remote_state = AuthorizationRemoteState::Unauthorized;
            return Ok(AuthorizationRemoteState::Unauthorized);
        }

        match cur_state.remote_state.clone() {
            AuthorizationRemoteState::Start => match action {
                AuthorizationRemoteAction::Connecting => {
                    if cur_state.remote_state
                        == AuthorizationRemoteState::TrustV0(TrustV0AuthorizationState::Connecting)
                    {
                        return Err(AuthorizationActionError::AlreadyConnecting);
                    };
                    // this state is not used in trust v0 so send to finished
                    cur_state.local_state = AuthorizationLocalState::AuthorizedAndComplete;
                    cur_state.remote_state =
                        AuthorizationRemoteState::TrustV0(TrustV0AuthorizationState::Connecting);
                    Ok(AuthorizationRemoteState::TrustV0(
                        TrustV0AuthorizationState::Connecting,
                    ))
                }
                #[cfg(feature = "trust-authorization")]
                AuthorizationRemoteAction::ReceiveAuthProtocolRequest => {
                    cur_state.remote_state = AuthorizationRemoteState::ReceivedAuthProtocolRequest;
                    Ok(AuthorizationRemoteState::ReceivedAuthProtocolRequest)
                }
                _ => Err(AuthorizationActionError::InvalidRemoteMessageOrder(
                    AuthorizationRemoteState::Start,
                    action,
                )),
            },
            #[cfg(feature = "trust-authorization")]
            AuthorizationRemoteState::ReceivedAuthProtocolRequest => match action {
                AuthorizationRemoteAction::SendAuthProtocolResponse => {
                    cur_state.remote_state = AuthorizationRemoteState::SentAuthProtocolResponse;
                    Ok(AuthorizationRemoteState::SentAuthProtocolResponse)
                }
                _ => Err(AuthorizationActionError::InvalidRemoteMessageOrder(
                    AuthorizationRemoteState::Start,
                    action,
                )),
            },
            AuthorizationRemoteState::TrustV0(state) => match action {
                AuthorizationRemoteAction::TrustV0(action) => {
                    let new_state = state.next_local_state(action)?;
                    cur_state.remote_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidRemoteMessageOrder(
                    AuthorizationRemoteState::TrustV0(state),
                    action,
                )),
            },
            // v1 state transitions
            #[cfg(feature = "trust-authorization")]
            AuthorizationRemoteState::SentAuthProtocolResponse => match action {
                #[cfg(feature = "trust-authorization")]
                AuthorizationRemoteAction::Trust(action) => {
                    let new_state = TrustAuthorizationRemoteState::TrustConnecting
                        .next_remote_state(action, cur_state)?;
                    cur_state.remote_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidRemoteMessageOrder(
                    AuthorizationRemoteState::SentAuthProtocolResponse,
                    action,
                )),
            },
            #[cfg(feature = "trust-authorization")]
            AuthorizationRemoteState::Trust(state) => match action {
                AuthorizationRemoteAction::Trust(action) => {
                    let new_state = state.next_remote_state(action, cur_state)?;
                    cur_state.remote_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidRemoteMessageOrder(
                    AuthorizationRemoteState::Trust(state),
                    action,
                )),
            },
            _ => Err(AuthorizationActionError::InvalidRemoteMessageOrder(
                cur_state.remote_state.clone(),
                action,
            )),
        }
    }

    #[cfg(feature = "trust-authorization")]
    pub(crate) fn received_complete(
        &self,
        connection_id: &str,
    ) -> Result<(), AuthorizationActionError> {
        let mut shared = self.shared.lock().map_err(|_| {
            AuthorizationActionError::InternalError("Authorization pool lock was poisoned".into())
        })?;

        let mut cur_state =
            shared
                .states
                .entry(connection_id.to_string())
                .or_insert(ManagedAuthorizationState {
                    local_state: AuthorizationLocalState::Start,
                    remote_state: AuthorizationRemoteState::Start,
                    received_complete: false,
                });

        cur_state.received_complete = true;

        if cur_state.local_state == AuthorizationLocalState::WaitForComplete {
            cur_state.local_state = AuthorizationLocalState::AuthorizedAndComplete;
        }
        Ok(())
    }
}
