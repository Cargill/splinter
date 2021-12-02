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
    ChallengeAuthorizationAcceptingAction, ChallengeAuthorizationAcceptingState,
    ChallengeAuthorizationInitiatingAction, ChallengeAuthorizationInitiatingState,
};
use self::trust_v0::{TrustV0AuthorizationAction, TrustV0AuthorizationState};
#[cfg(feature = "trust-authorization")]
use self::trust_v1::{
    TrustAuthorizationAcceptingAction, TrustAuthorizationAcceptingState,
    TrustAuthorizationInitiatingAction, TrustAuthorizationInitiatingState,
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
pub(crate) enum AuthorizationAcceptingState {
    Start,
    Done(Identity),
    Unauthorized,

    #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
    ReceivedAuthProtocolRequest,
    #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
    SentAuthProtocolResponse,

    TrustV0(TrustV0AuthorizationState),
    #[cfg(feature = "trust-authorization")]
    Trust(TrustAuthorizationAcceptingState),
    #[cfg(feature = "challenge-authorization")]
    Challenge(ChallengeAuthorizationAcceptingState),
}

impl fmt::Display for AuthorizationAcceptingState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AuthorizationAcceptingState::Start => f.write_str("Start"),
            AuthorizationAcceptingState::Done(_) => f.write_str("Done"),
            AuthorizationAcceptingState::Unauthorized => f.write_str("Unauthorized"),

            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
            AuthorizationAcceptingState::ReceivedAuthProtocolRequest => {
                f.write_str("ReceivedAuthProtocolRequest")
            }
            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
            AuthorizationAcceptingState::SentAuthProtocolResponse => {
                f.write_str("SentAuthProtocolResponse")
            }
            AuthorizationAcceptingState::TrustV0(state) => write!(f, "TrustV0: {}", state),
            #[cfg(feature = "trust-authorization")]
            AuthorizationAcceptingState::Trust(state) => write!(f, "Trust: {}", state),
            #[cfg(feature = "challenge-authorization")]
            AuthorizationAcceptingState::Challenge(state) => write!(f, "Challenge: {}", state),
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub(crate) enum AuthorizationInitiatingState {
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
    Trust(TrustAuthorizationInitiatingState),

    #[cfg(feature = "challenge-authorization")]
    Challenge(ChallengeAuthorizationInitiatingState),
}

impl fmt::Display for AuthorizationInitiatingState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AuthorizationInitiatingState::Start => f.write_str("Start"),
            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
            AuthorizationInitiatingState::Authorized => f.write_str("Authorized"),
            AuthorizationInitiatingState::Unauthorized => f.write_str("Unauthorized"),
            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
            AuthorizationInitiatingState::WaitingForAuthProtocolResponse => {
                f.write_str("WaitingForAuthProtocolResponse")
            }
            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
            AuthorizationInitiatingState::ReceivedAuthProtocolResponse => {
                f.write_str("ReceivedAuthProtocolResponse")
            }
            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
            AuthorizationInitiatingState::WaitForComplete => f.write_str("WaitForComplete"),
            AuthorizationInitiatingState::AuthorizedAndComplete => {
                f.write_str("AuthorizedAndComplete")
            }

            #[cfg(feature = "trust-authorization")]
            AuthorizationInitiatingState::Trust(action) => write!(f, "Trust: {}", action),
            #[cfg(feature = "challenge-authorization")]
            AuthorizationInitiatingState::Challenge(action) => write!(f, "Challenge: {}", action),
        }
    }
}

/// The state transitions that can be applied on a connection during authorization.
#[derive(PartialEq, Debug)]
pub(crate) enum AuthorizationAcceptingAction {
    Connecting,

    #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
    ReceiveAuthProtocolRequest,
    #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
    SendAuthProtocolResponse,

    TrustV0(TrustV0AuthorizationAction),
    #[cfg(feature = "trust-authorization")]
    Trust(TrustAuthorizationAcceptingAction),
    #[cfg(feature = "challenge-authorization")]
    Challenge(ChallengeAuthorizationAcceptingAction),

    Unauthorizing,
}

impl fmt::Display for AuthorizationAcceptingAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AuthorizationAcceptingAction::Connecting => f.write_str("Connecting"),
            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
            AuthorizationAcceptingAction::ReceiveAuthProtocolRequest => {
                f.write_str("ReceiveAuthProtocolRequest")
            }
            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
            AuthorizationAcceptingAction::SendAuthProtocolResponse => {
                f.write_str("SendAuthProtocolResponse")
            }
            AuthorizationAcceptingAction::TrustV0(action) => write!(f, "TrusV0t: {}", action),
            #[cfg(feature = "trust-authorization")]
            AuthorizationAcceptingAction::Trust(action) => write!(f, "Trust: {}", action),
            #[cfg(feature = "challenge-authorization")]
            AuthorizationAcceptingAction::Challenge(action) => write!(f, "Challenge: {}", action),

            AuthorizationAcceptingAction::Unauthorizing => f.write_str("Unauthorizing"),
        }
    }
}

/// The state transitions that can be applied on a connection during authorization.
#[derive(PartialEq, Debug)]
#[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
pub(crate) enum AuthorizationInitiatingAction {
    SendAuthProtocolRequest,
    ReceiveAuthProtocolResponse,

    #[cfg(feature = "trust-authorization")]
    Trust(TrustAuthorizationInitiatingAction),
    #[cfg(feature = "challenge-authorization")]
    Challenge(ChallengeAuthorizationInitiatingAction),

    SendAuthComplete,
    Unauthorizing,
}

#[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
impl fmt::Display for AuthorizationInitiatingAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AuthorizationInitiatingAction::SendAuthProtocolRequest => {
                f.write_str("SendAuthProtocolRequest")
            }
            AuthorizationInitiatingAction::ReceiveAuthProtocolResponse => {
                f.write_str("SendAuthProtocolResponse")
            }

            #[cfg(feature = "trust-authorization")]
            AuthorizationInitiatingAction::Trust(action) => write!(f, "Trust: {}", action),
            #[cfg(feature = "challenge-authorization")]
            AuthorizationInitiatingAction::Challenge(action) => write!(f, "Challenge: {}", action),
            AuthorizationInitiatingAction::SendAuthComplete => f.write_str("SendAuthComplete"),
            AuthorizationInitiatingAction::Unauthorizing => f.write_str("Unauthorizing"),
        }
    }
}

/// The errors that may occur for a connection during authorization.
#[derive(PartialEq, Debug)]
pub(crate) enum AuthorizationActionError {
    AlreadyConnecting,
    InvalidAcceptingMessageOrder(AuthorizationAcceptingState, AuthorizationAcceptingAction),
    #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
    InvalidInitiatingMessageOrder(AuthorizationInitiatingState, AuthorizationInitiatingAction),
    InternalError(String),
}

impl fmt::Display for AuthorizationActionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AuthorizationActionError::AlreadyConnecting => {
                f.write_str("Already attempting to connect")
            }
            AuthorizationActionError::InvalidAcceptingMessageOrder(start, action) => {
                write!(
                    f,
                    "Attempting to transition from remote state {} via {}",
                    start, action
                )
            }
            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
            AuthorizationActionError::InvalidInitiatingMessageOrder(start, action) => {
                write!(
                    f,
                    "Attempting to transition from locale state {} via {}",
                    start, action
                )
            }
            AuthorizationActionError::InternalError(msg) => f.write_str(msg),
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
    pub(crate) fn next_initiating_state(
        &self,
        connection_id: &str,
        action: AuthorizationInitiatingAction,
    ) -> Result<AuthorizationInitiatingState, AuthorizationActionError> {
        let mut shared = self.shared.lock().map_err(|_| {
            AuthorizationActionError::InternalError("Authorization pool lock was poisoned".into())
        })?;

        let mut cur_state =
            shared
                .states
                .entry(connection_id.to_string())
                .or_insert(ManagedAuthorizationState {
                    initiating_state: AuthorizationInitiatingState::Start,
                    accepting_state: AuthorizationAcceptingState::Start,
                    received_complete: false,
                    local_authorization: None,
                });

        if action == AuthorizationInitiatingAction::Unauthorizing {
            cur_state.initiating_state = AuthorizationInitiatingState::Unauthorized;
            cur_state.accepting_state = AuthorizationAcceptingState::Unauthorized;
            return Ok(AuthorizationInitiatingState::Unauthorized);
        }

        match cur_state.initiating_state.clone() {
            AuthorizationInitiatingState::Start => match action {
                AuthorizationInitiatingAction::SendAuthProtocolRequest => {
                    cur_state.initiating_state =
                        AuthorizationInitiatingState::WaitingForAuthProtocolResponse;
                    Ok(AuthorizationInitiatingState::WaitingForAuthProtocolResponse)
                }
                _ => Err(AuthorizationActionError::InvalidInitiatingMessageOrder(
                    AuthorizationInitiatingState::Start,
                    action,
                )),
            },
            // v1 state transitions
            AuthorizationInitiatingState::WaitingForAuthProtocolResponse => match action {
                AuthorizationInitiatingAction::ReceiveAuthProtocolResponse => {
                    cur_state.initiating_state =
                        AuthorizationInitiatingState::ReceivedAuthProtocolResponse;
                    Ok(AuthorizationInitiatingState::ReceivedAuthProtocolResponse)
                }
                _ => Err(AuthorizationActionError::InvalidInitiatingMessageOrder(
                    AuthorizationInitiatingState::Start,
                    action,
                )),
            },
            AuthorizationInitiatingState::ReceivedAuthProtocolResponse => match action {
                #[cfg(feature = "trust-authorization")]
                AuthorizationInitiatingAction::Trust(action) => {
                    let new_state = TrustAuthorizationInitiatingState::TrustConnecting
                        .next_initiating_state(action, cur_state)?;
                    Ok(new_state)
                }
                #[cfg(feature = "challenge-authorization")]
                AuthorizationInitiatingAction::Challenge(action) => {
                    let new_state = ChallengeAuthorizationInitiatingState::ChallengeConnecting
                        .next_initiating_state(action, cur_state)?;
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidInitiatingMessageOrder(
                    AuthorizationInitiatingState::WaitingForAuthProtocolResponse,
                    action,
                )),
            },
            #[cfg(feature = "trust-authorization")]
            AuthorizationInitiatingState::Trust(state) => match action {
                AuthorizationInitiatingAction::Trust(action) => {
                    let new_state = state.next_initiating_state(action, cur_state)?;
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidInitiatingMessageOrder(
                    AuthorizationInitiatingState::Trust(state),
                    action,
                )),
            },
            #[cfg(feature = "challenge-authorization")]
            AuthorizationInitiatingState::Challenge(state) => match action {
                AuthorizationInitiatingAction::Challenge(action) => {
                    let new_state = state.next_initiating_state(action, cur_state)?;
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidInitiatingMessageOrder(
                    AuthorizationInitiatingState::Challenge(state),
                    action,
                )),
            },
            AuthorizationInitiatingState::Authorized => match action {
                AuthorizationInitiatingAction::SendAuthComplete => {
                    let new_state = if cur_state.received_complete {
                        AuthorizationInitiatingState::AuthorizedAndComplete
                    } else {
                        AuthorizationInitiatingState::WaitForComplete
                    };

                    cur_state.initiating_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidInitiatingMessageOrder(
                    AuthorizationInitiatingState::Authorized,
                    action,
                )),
            },
            _ => Err(AuthorizationActionError::InvalidInitiatingMessageOrder(
                cur_state.initiating_state.clone(),
                action,
            )),
        }
    }

    /// Transitions from one authorization state to another. This is specific to the accepting node
    ///
    /// Errors
    ///
    /// The errors are error messages that should be returned on the appropriate message
    pub(crate) fn next_accepting_state(
        &self,
        connection_id: &str,
        action: AuthorizationAcceptingAction,
    ) -> Result<AuthorizationAcceptingState, AuthorizationActionError> {
        let mut shared = self.shared.lock().map_err(|_| {
            AuthorizationActionError::InternalError("Authorization pool lock was poisoned".into())
        })?;

        let mut cur_state =
            shared
                .states
                .entry(connection_id.to_string())
                .or_insert(ManagedAuthorizationState {
                    initiating_state: AuthorizationInitiatingState::Start,
                    accepting_state: AuthorizationAcceptingState::Start,
                    #[cfg(any(
                        feature = "trust-authorization",
                        feature = "challenge-authorization"
                    ))]
                    received_complete: false,
                    local_authorization: None,
                });

        if action == AuthorizationAcceptingAction::Unauthorizing {
            cur_state.initiating_state = AuthorizationInitiatingState::Unauthorized;
            cur_state.accepting_state = AuthorizationAcceptingState::Unauthorized;
            return Ok(AuthorizationAcceptingState::Unauthorized);
        }

        match cur_state.accepting_state.clone() {
            AuthorizationAcceptingState::Start => match action {
                AuthorizationAcceptingAction::Connecting => {
                    if cur_state.accepting_state
                        == AuthorizationAcceptingState::TrustV0(
                            TrustV0AuthorizationState::Connecting,
                        )
                    {
                        return Err(AuthorizationActionError::AlreadyConnecting);
                    };
                    // this state is not used in trust v0 so send to finished
                    cur_state.initiating_state =
                        AuthorizationInitiatingState::AuthorizedAndComplete;
                    cur_state.accepting_state =
                        AuthorizationAcceptingState::TrustV0(TrustV0AuthorizationState::Connecting);
                    Ok(AuthorizationAcceptingState::TrustV0(
                        TrustV0AuthorizationState::Connecting,
                    ))
                }
                #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
                AuthorizationAcceptingAction::ReceiveAuthProtocolRequest => {
                    cur_state.accepting_state =
                        AuthorizationAcceptingState::ReceivedAuthProtocolRequest;
                    Ok(AuthorizationAcceptingState::ReceivedAuthProtocolRequest)
                }
                _ => Err(AuthorizationActionError::InvalidAcceptingMessageOrder(
                    AuthorizationAcceptingState::Start,
                    action,
                )),
            },
            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
            AuthorizationAcceptingState::ReceivedAuthProtocolRequest => match action {
                AuthorizationAcceptingAction::SendAuthProtocolResponse => {
                    cur_state.accepting_state =
                        AuthorizationAcceptingState::SentAuthProtocolResponse;
                    Ok(AuthorizationAcceptingState::SentAuthProtocolResponse)
                }
                _ => Err(AuthorizationActionError::InvalidAcceptingMessageOrder(
                    AuthorizationAcceptingState::Start,
                    action,
                )),
            },
            AuthorizationAcceptingState::TrustV0(state) => match action {
                AuthorizationAcceptingAction::TrustV0(action) => {
                    let new_state = state.next_initiating_state(action)?;
                    cur_state.accepting_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidAcceptingMessageOrder(
                    AuthorizationAcceptingState::TrustV0(state),
                    action,
                )),
            },
            // v1 state transitions
            #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
            AuthorizationAcceptingState::SentAuthProtocolResponse => match action {
                #[cfg(feature = "trust-authorization")]
                AuthorizationAcceptingAction::Trust(action) => {
                    let new_state = TrustAuthorizationAcceptingState::TrustConnecting
                        .next_accepting_state(action, cur_state)?;
                    cur_state.accepting_state = new_state.clone();
                    Ok(new_state)
                }
                #[cfg(feature = "challenge-authorization")]
                AuthorizationAcceptingAction::Challenge(action) => {
                    let new_state = ChallengeAuthorizationAcceptingState::ChallengeConnecting
                        .next_accepting_state(action, cur_state)?;
                    cur_state.accepting_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidAcceptingMessageOrder(
                    AuthorizationAcceptingState::SentAuthProtocolResponse,
                    action,
                )),
            },
            #[cfg(feature = "trust-authorization")]
            AuthorizationAcceptingState::Trust(state) => match action {
                AuthorizationAcceptingAction::Trust(action) => {
                    let new_state = state.next_accepting_state(action, cur_state)?;
                    cur_state.accepting_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidAcceptingMessageOrder(
                    AuthorizationAcceptingState::Trust(state),
                    action,
                )),
            },
            #[cfg(feature = "challenge-authorization")]
            AuthorizationAcceptingState::Challenge(state) => match action {
                AuthorizationAcceptingAction::Challenge(action) => {
                    let new_state = state.next_accepting_state(action, cur_state)?;
                    cur_state.accepting_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidAcceptingMessageOrder(
                    AuthorizationAcceptingState::Challenge(state),
                    action,
                )),
            },
            _ => Err(AuthorizationActionError::InvalidAcceptingMessageOrder(
                cur_state.accepting_state.clone(),
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
                    initiating_state: AuthorizationInitiatingState::Start,
                    accepting_state: AuthorizationAcceptingState::Start,
                    received_complete: false,
                    local_authorization: None,
                });

        cur_state.received_complete = true;

        if cur_state.initiating_state == AuthorizationInitiatingState::WaitForComplete {
            cur_state.initiating_state = AuthorizationInitiatingState::AuthorizedAndComplete;
        }
        Ok(())
    }

    pub(crate) fn set_local_authorization(
        &self,
        connection_id: &str,
        identity: Identity,
    ) -> Result<(), AuthorizationActionError> {
        let mut shared = self.shared.lock().map_err(|_| {
            AuthorizationActionError::InternalError("Authorization pool lock was poisoned".into())
        })?;

        let mut cur_state =
            shared
                .states
                .entry(connection_id.to_string())
                .or_insert(ManagedAuthorizationState {
                    initiating_state: AuthorizationInitiatingState::Start,
                    accepting_state: AuthorizationAcceptingState::Start,
                    #[cfg(any(
                        feature = "trust-authorization",
                        feature = "challenge-authorization"
                    ))]
                    received_complete: false,
                    local_authorization: None,
                });

        cur_state.local_authorization = Some(identity);
        Ok(())
    }
}
