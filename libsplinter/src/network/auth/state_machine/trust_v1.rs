// Copyright 2018-2022 Cargill Incorporated
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

use std::fmt;

use crate::network::auth::ManagedAuthorizationState;

use super::{
    AuthorizationAcceptingAction, AuthorizationAcceptingState, AuthorizationActionError,
    AuthorizationInitiatingAction, AuthorizationInitiatingState, Identity,
};

#[derive(PartialEq, Debug, Clone)]
pub(crate) enum TrustAuthorizationInitiatingState {
    TrustConnecting,
    WaitingForAuthTrustResponse,
}

impl fmt::Display for TrustAuthorizationInitiatingState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            TrustAuthorizationInitiatingState::TrustConnecting => "TrustConnecting",
            TrustAuthorizationInitiatingState::WaitingForAuthTrustResponse => {
                "WaitingForAuthTrustResponse"
            }
        })
    }
}

#[derive(PartialEq, Debug, Clone)]
pub(crate) enum TrustAuthorizationAcceptingState {
    TrustConnecting,
    ReceivedAuthTrustRequest(Identity),
}

impl fmt::Display for TrustAuthorizationAcceptingState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            TrustAuthorizationAcceptingState::TrustConnecting => "TrustConnecting",
            TrustAuthorizationAcceptingState::ReceivedAuthTrustRequest(_) => {
                "ReceiveAuthTrustRequest"
            }
        })
    }
}

/// The state transitions that can be applied on a connection during authorization.
#[derive(PartialEq, Debug)]
pub(crate) enum TrustAuthorizationAcceptingAction {
    ReceiveAuthTrustRequest(Identity),
    SendAuthTrustResponse,
}

impl fmt::Display for TrustAuthorizationAcceptingAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TrustAuthorizationAcceptingAction::ReceiveAuthTrustRequest(_) => {
                f.write_str("ReceiveAuthTrustRequest")
            }
            TrustAuthorizationAcceptingAction::SendAuthTrustResponse => {
                f.write_str("SendAuthTrustResponse")
            }
        }
    }
}

/// The state transitions that can be applied on a connection during authorization.
#[derive(PartialEq, Debug)]
pub(crate) enum TrustAuthorizationInitiatingAction {
    SendAuthTrustRequest(Identity),
    ReceiveAuthTrustResponse,
}

impl fmt::Display for TrustAuthorizationInitiatingAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TrustAuthorizationInitiatingAction::SendAuthTrustRequest(_) => {
                f.write_str("SendAuthTrustRequest")
            }
            TrustAuthorizationInitiatingAction::ReceiveAuthTrustResponse => {
                f.write_str("ReceiveAuthTrustResponse")
            }
        }
    }
}

impl TrustAuthorizationInitiatingState {
    /// Transitions from one authorization state to another
    ///
    /// Errors
    ///
    /// The errors are error messages that should be returned on the appropriate message
    pub(crate) fn next_initiating_state(
        &self,
        action: TrustAuthorizationInitiatingAction,
        cur_state: &mut ManagedAuthorizationState,
    ) -> Result<AuthorizationInitiatingState, AuthorizationActionError> {
        match &self {
            TrustAuthorizationInitiatingState::TrustConnecting => match action {
                TrustAuthorizationInitiatingAction::SendAuthTrustRequest(identity) => {
                    cur_state.local_authorization = Some(identity);
                    let new_state = AuthorizationInitiatingState::Trust(
                        TrustAuthorizationInitiatingState::WaitingForAuthTrustResponse,
                    );
                    cur_state.initiating_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidInitiatingMessageOrder(
                    AuthorizationInitiatingState::Trust(self.clone()),
                    AuthorizationInitiatingAction::Trust(action),
                )),
            },
            TrustAuthorizationInitiatingState::WaitingForAuthTrustResponse => match action {
                TrustAuthorizationInitiatingAction::ReceiveAuthTrustResponse => {
                    let new_state = AuthorizationInitiatingState::Authorized;
                    cur_state.initiating_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidInitiatingMessageOrder(
                    AuthorizationInitiatingState::Trust(self.clone()),
                    AuthorizationInitiatingAction::Trust(action),
                )),
            },
        }
    }
}

impl TrustAuthorizationAcceptingState {
    /// Transitions from one authorization state to another
    ///
    /// Errors
    ///
    /// The errors are error messages that should be returned on the appropriate message
    pub(crate) fn next_accepting_state(
        &self,
        action: TrustAuthorizationAcceptingAction,
        cur_state: &mut ManagedAuthorizationState,
    ) -> Result<AuthorizationAcceptingState, AuthorizationActionError> {
        match &self {
            TrustAuthorizationAcceptingState::TrustConnecting => match action {
                TrustAuthorizationAcceptingAction::ReceiveAuthTrustRequest(identity) => {
                    let new_state = AuthorizationAcceptingState::Trust(
                        TrustAuthorizationAcceptingState::ReceivedAuthTrustRequest(identity),
                    );
                    cur_state.accepting_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidAcceptingMessageOrder(
                    AuthorizationAcceptingState::Trust(self.clone()),
                    AuthorizationAcceptingAction::Trust(action),
                )),
            },
            TrustAuthorizationAcceptingState::ReceivedAuthTrustRequest(identity) => match action {
                TrustAuthorizationAcceptingAction::SendAuthTrustResponse => {
                    cur_state.accepting_state = AuthorizationAcceptingState::Done(identity.clone());
                    Ok(AuthorizationAcceptingState::Done(identity.clone()))
                }
                _ => Err(AuthorizationActionError::InvalidAcceptingMessageOrder(
                    AuthorizationAcceptingState::Trust(self.clone()),
                    AuthorizationAcceptingAction::Trust(action),
                )),
            },
        }
    }
}
