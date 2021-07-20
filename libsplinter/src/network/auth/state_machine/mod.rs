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
#[cfg(feature = "challenge-authorization")]
pub mod challenge_v1;
pub mod trust_v0;
#[cfg(feature = "trust-authorization")]
pub mod trust_v1;

use std::fmt;
use std::sync::{Arc, Mutex};

#[cfg(feature = "challenge-authorization")]
use crate::public_key::PublicKey;

#[cfg(feature = "challenge-authorization")]
use self::challenge_v1::{
    ChallengeAuthorizationLocalAction, ChallengeAuthorizationLocalState,
    ChallengeAuthorizationRemoteAction, ChallengeAuthorizationRemoteState,
};
use self::trust_v0::{TrustV0AuthorizationAction, TrustV0AuthorizationState};
#[cfg(feature = "trust-authorization")]
use self::trust_v1::{
    TrustAuthorizationLocalAction, TrustAuthorizationLocalState, TrustAuthorizationRemoteAction,
    TrustAuthorizationRemoteState,
};

use super::{ManagedAuthorizationState, ManagedAuthorizations};

#[derive(Debug, PartialEq, Clone)]
pub enum Identity {
    Trust {
        identity: String,
    },
    #[cfg(feature = "challenge-authorization")]
    Challenge {
        public_key: PublicKey,
    },
}

#[derive(PartialEq, Debug, Clone)]
pub(crate) enum AuthorizationRemoteState {
    Start,
    Done(Identity),
    Unauthorized,

    #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
    ReceivedAuthProtocolRequest,
    #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
    SentAuthProtocolResponse,

    TrustV0(TrustV0AuthorizationState),
    #[cfg(feature = "trust-authorization")]
    Trust(TrustAuthorizationRemoteState),
    #[cfg(feature = "challenge-authorization")]
    Challenge(ChallengeAuthorizationRemoteState),
}

impl fmt::Display for AuthorizationRemoteState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AuthorizationRemoteState::Start => f.write_str("Start"),
            AuthorizationRemoteState::Done(_) => f.write_str("Done"),
            AuthorizationRemoteState::Unauthorized => f.write_str("Unauthorized"),

            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
            AuthorizationRemoteState::ReceivedAuthProtocolRequest => {
                f.write_str("ReceivedAuthProtocolRequest")
            }
            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
            AuthorizationRemoteState::SentAuthProtocolResponse => {
                f.write_str("SentAuthProtocolResponse")
            }
            AuthorizationRemoteState::TrustV0(state) => write!(f, "TrustV0: {}", state),
            #[cfg(feature = "trust-authorization")]
            AuthorizationRemoteState::Trust(state) => write!(f, "Trust: {}", state),
            #[cfg(feature = "challenge-authorization")]
            AuthorizationRemoteState::Challenge(state) => write!(f, "Challenge: {}", state),
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub(crate) enum AuthorizationLocalState {
    Start,
    #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
    WaitingForAuthProtocolResponse,
    #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
    ReceivedAuthProtocolResponse,
    #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
    Authorized,
    #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
    WaitForComplete,
    AuthorizedAndComplete,
    Unauthorized,

    #[cfg(feature = "trust-authorization")]
    Trust(TrustAuthorizationLocalState),

    #[cfg(feature = "challenge-authorization")]
    Challenge(ChallengeAuthorizationLocalState),
}

impl fmt::Display for AuthorizationLocalState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AuthorizationLocalState::Start => f.write_str("Start"),
            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
            AuthorizationLocalState::Authorized => f.write_str("Authorized"),
            AuthorizationLocalState::Unauthorized => f.write_str("Unauthorized"),
            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
            AuthorizationLocalState::WaitingForAuthProtocolResponse => {
                f.write_str("WaitingForAuthProtocolResponse")
            }
            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
            AuthorizationLocalState::ReceivedAuthProtocolResponse => {
                f.write_str("ReceivedAuthProtocolResponse")
            }
            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
            AuthorizationLocalState::WaitForComplete => f.write_str("WaitForComplete"),
            AuthorizationLocalState::AuthorizedAndComplete => f.write_str("AuthorizedAndComplete"),

            #[cfg(feature = "trust-authorization")]
            AuthorizationLocalState::Trust(action) => write!(f, "Trust: {}", action),
            #[cfg(feature = "challenge-authorization")]
            AuthorizationLocalState::Challenge(action) => write!(f, "Challenge: {}", action),
        }
    }
}

/// The state transitions that can be applied on a connection during authorization.
#[derive(PartialEq, Debug)]
pub(crate) enum AuthorizationRemoteAction {
    Connecting,

    #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
    ReceiveAuthProtocolRequest,
    #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
    SendAuthProtocolResponse,

    TrustV0(TrustV0AuthorizationAction),
    #[cfg(feature = "trust-authorization")]
    Trust(TrustAuthorizationRemoteAction),
    #[cfg(feature = "challenge-authorization")]
    Challenge(ChallengeAuthorizationRemoteAction),

    Unauthorizing,
}

impl fmt::Display for AuthorizationRemoteAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AuthorizationRemoteAction::Connecting => f.write_str("Connecting"),
            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
            AuthorizationRemoteAction::ReceiveAuthProtocolRequest => {
                f.write_str("ReceiveAuthProtocolRequest")
            }
            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
            AuthorizationRemoteAction::SendAuthProtocolResponse => {
                f.write_str("SendAuthProtocolResponse")
            }
            AuthorizationRemoteAction::TrustV0(action) => write!(f, "TrusV0t: {}", action),
            #[cfg(feature = "trust-authorization")]
            AuthorizationRemoteAction::Trust(action) => write!(f, "Trust: {}", action),
            #[cfg(feature = "challenge-authorization")]
            AuthorizationRemoteAction::Challenge(action) => write!(f, "Challenge: {}", action),

            AuthorizationRemoteAction::Unauthorizing => f.write_str("Unauthorizing"),
        }
    }
}

/// The state transitions that can be applied on a connection during authorization.
#[derive(PartialEq, Debug)]
#[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
pub(crate) enum AuthorizationLocalAction {
    SendAuthProtocolRequest,
    ReceiveAuthProtocolResponse,

    #[cfg(feature = "trust-authorization")]
    Trust(TrustAuthorizationLocalAction),
    #[cfg(feature = "challenge-authorization")]
    Challenge(ChallengeAuthorizationLocalAction),

    SendAuthComplete,
    Unauthorizing,
}

#[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
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
            #[cfg(feature = "challenge-authorization")]
            AuthorizationLocalAction::Challenge(action) => write!(f, "Challenge: {}", action),
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
    #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
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
            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
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
    #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
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
                #[cfg(feature = "challenge-authorization")]
                AuthorizationLocalAction::Challenge(action) => {
                    let new_state = ChallengeAuthorizationLocalState::ChallengeConnecting
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
            #[cfg(feature = "challenge-authorization")]
            AuthorizationLocalState::Challenge(state) => match action {
                AuthorizationLocalAction::Challenge(action) => {
                    let new_state = state.next_local_state(action, cur_state)?;
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidLocalMessageOrder(
                    AuthorizationLocalState::Challenge(state),
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
                #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
                AuthorizationRemoteAction::ReceiveAuthProtocolRequest => {
                    cur_state.remote_state = AuthorizationRemoteState::ReceivedAuthProtocolRequest;
                    Ok(AuthorizationRemoteState::ReceivedAuthProtocolRequest)
                }
                _ => Err(AuthorizationActionError::InvalidRemoteMessageOrder(
                    AuthorizationRemoteState::Start,
                    action,
                )),
            },
            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
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
            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
            AuthorizationRemoteState::SentAuthProtocolResponse => match action {
                #[cfg(feature = "trust-authorization")]
                AuthorizationRemoteAction::Trust(action) => {
                    let new_state = TrustAuthorizationRemoteState::TrustConnecting
                        .next_remote_state(action, cur_state)?;
                    cur_state.remote_state = new_state.clone();
                    Ok(new_state)
                }
                #[cfg(feature = "challenge-authorization")]
                AuthorizationRemoteAction::Challenge(action) => {
                    let new_state = ChallengeAuthorizationRemoteState::ChallengeConnecting
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
            #[cfg(feature = "challenge-authorization")]
            AuthorizationRemoteState::Challenge(state) => match action {
                AuthorizationRemoteAction::Challenge(action) => {
                    let new_state = state.next_remote_state(action, cur_state)?;
                    cur_state.remote_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidRemoteMessageOrder(
                    AuthorizationRemoteState::Challenge(state),
                    action,
                )),
            },
            _ => Err(AuthorizationActionError::InvalidRemoteMessageOrder(
                cur_state.remote_state.clone(),
                action,
            )),
        }
    }

    #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
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
