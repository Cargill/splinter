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

use super::{AuthorizationAction, AuthorizationActionError, AuthorizationState, Identity};

#[derive(PartialEq, Debug, Clone)]
pub(crate) enum TrustAuthorizationState {
    TrustConnecting,
    Identified(String),
    Authorized(String),
}

impl fmt::Display for TrustAuthorizationState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            TrustAuthorizationState::TrustConnecting => "Connecting",
            TrustAuthorizationState::Identified(_) => "Trust Identified",
            TrustAuthorizationState::Authorized(_) => "Authorized",
        })
    }
}

/// The state transitions that can be applied on a connection during authorization.
#[derive(PartialEq, Debug)]
pub(crate) enum TrustAuthorizationAction {
    TrustIdentifying(Identity),
    Authorizing,
}

impl fmt::Display for TrustAuthorizationAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TrustAuthorizationAction::TrustIdentifying(_) => f.write_str("TrustIdentifying"),
            TrustAuthorizationAction::Authorizing => f.write_str("Authorizing"),
        }
    }
}

impl TrustAuthorizationState {
    /// Transitions from one authorization state to another
    ///
    /// Errors
    ///
    /// The errors are error messages that should be returned on the appropriate message
    pub(crate) fn next_state(
        &self,
        action: TrustAuthorizationAction,
        cur_state: &mut ManagedAuthorizationState,
    ) -> Result<AuthorizationState, AuthorizationActionError> {
        match &self {
            TrustAuthorizationState::TrustConnecting => match action {
                TrustAuthorizationAction::TrustIdentifying(identity) => {
                    let new_state =
                        AuthorizationState::Trust(TrustAuthorizationState::Identified(identity));
                    cur_state.local_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidMessageOrder(
                    AuthorizationState::Trust(self.clone()),
                    AuthorizationAction::Trust(action),
                )),
            },
            TrustAuthorizationState::Identified(identity) => match action {
                TrustAuthorizationAction::Authorizing => {
                    let new_state = {
                        match &cur_state.remote_state {
                            AuthorizationState::Trust(TrustAuthorizationState::Authorized(
                                local_id,
                            )) => {
                                cur_state.remote_state =
                                    AuthorizationState::AuthComplete(Some(local_id.to_string()));
                                AuthorizationState::AuthComplete(Some(identity.to_string()))
                            }
                            _ => AuthorizationState::Trust(TrustAuthorizationState::Authorized(
                                identity.to_string(),
                            )),
                        }
                    };

                    cur_state.local_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidMessageOrder(
                    AuthorizationState::Trust(self.clone()),
                    AuthorizationAction::Trust(action),
                )),
            },
            _ => Err(AuthorizationActionError::InvalidMessageOrder(
                AuthorizationState::Trust(self.clone()),
                AuthorizationAction::Trust(action),
            )),
        }
    }

    /// Transitions from one authorization state to another
    ///
    /// Errors
    ///
    /// The errors are error messages that should be returned on the appropriate message
    pub(crate) fn next_remote_state(
        &self,
        action: TrustAuthorizationAction,
        cur_state: &mut ManagedAuthorizationState,
    ) -> Result<AuthorizationState, AuthorizationActionError> {
        match &self {
            TrustAuthorizationState::TrustConnecting => match action {
                TrustAuthorizationAction::TrustIdentifying(identity) => {
                    let new_state =
                        AuthorizationState::Trust(TrustAuthorizationState::Identified(identity));
                    cur_state.remote_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidMessageOrder(
                    AuthorizationState::Trust(self.clone()),
                    AuthorizationAction::Trust(action),
                )),
            },
            TrustAuthorizationState::Identified(identity) => match action {
                TrustAuthorizationAction::Authorizing => {
                    let new_state = {
                        match &cur_state.local_state {
                            AuthorizationState::Trust(TrustAuthorizationState::Authorized(
                                local_id,
                            )) => {
                                cur_state.local_state =
                                    AuthorizationState::AuthComplete(Some(local_id.to_string()));
                                AuthorizationState::AuthComplete(Some(identity.to_string()))
                            }
                            _ => AuthorizationState::Trust(TrustAuthorizationState::Authorized(
                                identity.to_string(),
                            )),
                        }
                    };

                    cur_state.remote_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidMessageOrder(
                    AuthorizationState::Trust(self.clone()),
                    AuthorizationAction::Trust(action),
                )),
            },
            _ => Err(AuthorizationActionError::InvalidMessageOrder(
                AuthorizationState::Trust(self.clone()),
                AuthorizationAction::Trust(action),
            )),
        }
    }
}
