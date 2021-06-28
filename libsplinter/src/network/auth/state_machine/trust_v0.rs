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

use super::{AuthorizationAction, AuthorizationActionError, AuthorizationState, Identity};

/// The states of a connection during v0 trust authorization.
#[derive(PartialEq, Debug, Clone)]
pub(crate) enum TrustV0AuthorizationState {
    Connecting,
    RemoteIdentified(Identity),
    RemoteAccepted,
}

impl fmt::Display for TrustV0AuthorizationState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            TrustV0AuthorizationState::Connecting => "Connecting",
            TrustV0AuthorizationState::RemoteIdentified(_) => "Remote Identified",
            TrustV0AuthorizationState::RemoteAccepted => "Remote Accepted",
        })
    }
}

#[derive(PartialEq, Debug)]
pub(crate) enum TrustV0AuthorizationAction {
    TrustIdentifyingV0(Identity),
    RemoteAuthorizing,
}

impl fmt::Display for TrustV0AuthorizationAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TrustV0AuthorizationAction::TrustIdentifyingV0(_) => f.write_str("TrustIdentifyingV0"),
            TrustV0AuthorizationAction::RemoteAuthorizing => f.write_str("RemoteAuthorizing"),
        }
    }
}

impl TrustV0AuthorizationState {
    /// Transitions from one authorization state to another
    ///
    /// Errors
    ///
    /// The errors are error messages that should be returned on the appropriate message
    pub(crate) fn next_state(
        &self,
        action: TrustV0AuthorizationAction,
    ) -> Result<AuthorizationState, AuthorizationActionError> {
        match &self {
            // v0 state transitions
            TrustV0AuthorizationState::Connecting => match action {
                TrustV0AuthorizationAction::TrustIdentifyingV0(identity) => {
                    Ok(AuthorizationState::TrustV0(
                        TrustV0AuthorizationState::RemoteIdentified(identity),
                    ))
                }
                TrustV0AuthorizationAction::RemoteAuthorizing => Ok(AuthorizationState::TrustV0(
                    TrustV0AuthorizationState::RemoteAccepted,
                )),
            },
            TrustV0AuthorizationState::RemoteIdentified(identity) => match action {
                TrustV0AuthorizationAction::RemoteAuthorizing => {
                    Ok(AuthorizationState::AuthComplete(Some(identity.clone())))
                }
                _ => Err(AuthorizationActionError::InvalidMessageOrder(
                    AuthorizationState::TrustV0(TrustV0AuthorizationState::RemoteIdentified(
                        identity.clone(),
                    )),
                    AuthorizationAction::TrustV0(action),
                )),
            },
            TrustV0AuthorizationState::RemoteAccepted => match action {
                TrustV0AuthorizationAction::TrustIdentifyingV0(identity) => {
                    Ok(AuthorizationState::AuthComplete(Some(identity)))
                }
                _ => Err(AuthorizationActionError::InvalidMessageOrder(
                    AuthorizationState::TrustV0(TrustV0AuthorizationState::RemoteAccepted),
                    AuthorizationAction::TrustV0(action),
                )),
            },
        }
    }
}
