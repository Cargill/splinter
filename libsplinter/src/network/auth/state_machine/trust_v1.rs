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

use std::fmt;

use crate::network::auth::ManagedAuthorizationState;

use super::{
    AuthorizationActionError, AuthorizationLocalAction, AuthorizationLocalState,
    AuthorizationRemoteAction, AuthorizationRemoteState, Identity,
};

#[derive(PartialEq, Debug, Clone)]
pub(crate) enum TrustAuthorizationLocalState {
    TrustConnecting,
    WaitingForAuthTrustResponse,
}

impl fmt::Display for TrustAuthorizationLocalState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            TrustAuthorizationLocalState::TrustConnecting => "TrustConnecting",
            TrustAuthorizationLocalState::WaitingForAuthTrustResponse => {
                "WaitingForAuthTrustResponse"
            }
        })
    }
}

#[derive(PartialEq, Debug, Clone)]
pub(crate) enum TrustAuthorizationRemoteState {
    TrustConnecting,
    ReceivedAuthTrustRequest(Identity),
}

impl fmt::Display for TrustAuthorizationRemoteState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            TrustAuthorizationRemoteState::TrustConnecting => "TrustConnecting",
            TrustAuthorizationRemoteState::ReceivedAuthTrustRequest(_) => "ReceiveAuthTrustRequest",
        })
    }
}

/// The state transitions that can be applied on a connection during authorization.
#[derive(PartialEq, Debug)]
pub(crate) enum TrustAuthorizationRemoteAction {
    ReceiveAuthTrustRequest(Identity),
    SendAuthTrustResponse,
}

impl fmt::Display for TrustAuthorizationRemoteAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TrustAuthorizationRemoteAction::ReceiveAuthTrustRequest(_) => {
                f.write_str("ReceiveAuthTrustRequest")
            }
            TrustAuthorizationRemoteAction::SendAuthTrustResponse => {
                f.write_str("SendAuthTrustResponse")
            }
        }
    }
}

/// The state transitions that can be applied on a connection during authorization.
#[derive(PartialEq, Debug)]
pub(crate) enum TrustAuthorizationLocalAction {
    SendAuthTrustRequest(Identity),
    ReceiveAuthTrustResponse,
}

impl fmt::Display for TrustAuthorizationLocalAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TrustAuthorizationLocalAction::SendAuthTrustRequest(_) => {
                f.write_str("SendAuthTrustRequest")
            }
            TrustAuthorizationLocalAction::ReceiveAuthTrustResponse => {
                f.write_str("ReceiveAuthTrustResponse")
            }
        }
    }
}

impl TrustAuthorizationLocalState {
    /// Transitions from one authorization state to another
    ///
    /// Errors
    ///
    /// The errors are error messages that should be returned on the appropriate message
    pub(crate) fn next_local_state(
        &self,
        action: TrustAuthorizationLocalAction,
        cur_state: &mut ManagedAuthorizationState,
    ) -> Result<AuthorizationLocalState, AuthorizationActionError> {
        match &self {
            TrustAuthorizationLocalState::TrustConnecting => match action {
                TrustAuthorizationLocalAction::SendAuthTrustRequest(identity) => {
                    cur_state.local_authorization = Some(identity);
                    let new_state = AuthorizationLocalState::Trust(
                        TrustAuthorizationLocalState::WaitingForAuthTrustResponse,
                    );
                    cur_state.local_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidLocalMessageOrder(
                    AuthorizationLocalState::Trust(self.clone()),
                    AuthorizationLocalAction::Trust(action),
                )),
            },
            TrustAuthorizationLocalState::WaitingForAuthTrustResponse => match action {
                TrustAuthorizationLocalAction::ReceiveAuthTrustResponse => {
                    let new_state = AuthorizationLocalState::Authorized;
                    cur_state.local_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidLocalMessageOrder(
                    AuthorizationLocalState::Trust(self.clone()),
                    AuthorizationLocalAction::Trust(action),
                )),
            },
        }
    }
}

impl TrustAuthorizationRemoteState {
    /// Transitions from one authorization state to another
    ///
    /// Errors
    ///
    /// The errors are error messages that should be returned on the appropriate message
    pub(crate) fn next_remote_state(
        &self,
        action: TrustAuthorizationRemoteAction,
        cur_state: &mut ManagedAuthorizationState,
    ) -> Result<AuthorizationRemoteState, AuthorizationActionError> {
        match &self {
            TrustAuthorizationRemoteState::TrustConnecting => match action {
                TrustAuthorizationRemoteAction::ReceiveAuthTrustRequest(identity) => {
                    let new_state = AuthorizationRemoteState::Trust(
                        TrustAuthorizationRemoteState::ReceivedAuthTrustRequest(identity),
                    );
                    cur_state.remote_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidRemoteMessageOrder(
                    AuthorizationRemoteState::Trust(self.clone()),
                    AuthorizationRemoteAction::Trust(action),
                )),
            },
            TrustAuthorizationRemoteState::ReceivedAuthTrustRequest(identity) => match action {
                TrustAuthorizationRemoteAction::SendAuthTrustResponse => {
                    cur_state.remote_state = AuthorizationRemoteState::Done(identity.clone());
                    Ok(AuthorizationRemoteState::Done(identity.clone()))
                }
                _ => Err(AuthorizationActionError::InvalidRemoteMessageOrder(
                    AuthorizationRemoteState::Trust(self.clone()),
                    AuthorizationRemoteAction::Trust(action),
                )),
            },
        }
    }
}
