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

use crate::network::auth::state_machine::{
    AuthorizationActionError, AuthorizationRemoteAction, AuthorizationRemoteState, Identity,
};

use super::ChallengeAuthorizationRemoteAction;

#[derive(PartialEq, Debug, Clone)]
pub(crate) enum ChallengeAuthorizationRemoteState {
    ChallengeConnecting,
    ReceivedAuthChallengeNonce,
    WaitingForAuthChallengeSubmitRequest,
    ReceivedAuthChallengeSubmitRequest(Identity),
}

impl ChallengeAuthorizationRemoteState {
    /// Transitions from one authorization state to another
    ///
    /// Errors
    ///
    /// The errors are error messages that should be returned on the appropriate message
    pub(crate) fn next_remote_state(
        &self,
        action: ChallengeAuthorizationRemoteAction,
        cur_state: &mut ManagedAuthorizationState,
    ) -> Result<AuthorizationRemoteState, AuthorizationActionError> {
        match &self {
            ChallengeAuthorizationRemoteState::ChallengeConnecting => match action {
                ChallengeAuthorizationRemoteAction::ReceiveAuthChallengeNonceRequest => {
                    let new_state = AuthorizationRemoteState::Challenge(
                        ChallengeAuthorizationRemoteState::ReceivedAuthChallengeNonce,
                    );
                    cur_state.remote_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidRemoteMessageOrder(
                    AuthorizationRemoteState::Challenge(self.clone()),
                    AuthorizationRemoteAction::Challenge(action),
                )),
            },
            ChallengeAuthorizationRemoteState::ReceivedAuthChallengeNonce => match action {
                ChallengeAuthorizationRemoteAction::SendAuthChallengeNonceResponse => {
                    let new_state = AuthorizationRemoteState::Challenge(
                        ChallengeAuthorizationRemoteState::WaitingForAuthChallengeSubmitRequest,
                    );
                    cur_state.remote_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidRemoteMessageOrder(
                    AuthorizationRemoteState::Challenge(self.clone()),
                    AuthorizationRemoteAction::Challenge(action),
                )),
            },
            ChallengeAuthorizationRemoteState::WaitingForAuthChallengeSubmitRequest => match action
            {
                ChallengeAuthorizationRemoteAction::ReceiveAuthChallengeSubmitRequest(identity) => {
                    let new_state = AuthorizationRemoteState::Challenge(
                        ChallengeAuthorizationRemoteState::ReceivedAuthChallengeSubmitRequest(
                            identity,
                        ),
                    );
                    cur_state.remote_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidRemoteMessageOrder(
                    AuthorizationRemoteState::Challenge(self.clone()),
                    AuthorizationRemoteAction::Challenge(action),
                )),
            },
            ChallengeAuthorizationRemoteState::ReceivedAuthChallengeSubmitRequest(identity) => {
                match action {
                    ChallengeAuthorizationRemoteAction::SendAuthChallengeSubmitResponse => {
                        let new_state = AuthorizationRemoteState::Done(identity.clone());
                        cur_state.remote_state = new_state.clone();
                        Ok(new_state)
                    }
                    _ => Err(AuthorizationActionError::InvalidRemoteMessageOrder(
                        AuthorizationRemoteState::Challenge(self.clone()),
                        AuthorizationRemoteAction::Challenge(action),
                    )),
                }
            }
        }
    }
}

impl fmt::Display for ChallengeAuthorizationRemoteState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            ChallengeAuthorizationRemoteState::ChallengeConnecting => "ChallengeConnecting",
            ChallengeAuthorizationRemoteState::ReceivedAuthChallengeNonce => {
                "ReceivedAuthChallengeNonce"
            }
            ChallengeAuthorizationRemoteState::WaitingForAuthChallengeSubmitRequest => {
                "WaitingForAuthChallengeSubmitRequest"
            }
            ChallengeAuthorizationRemoteState::ReceivedAuthChallengeSubmitRequest(_) => {
                "ReceivedAuthChallengeSubmitRequest"
            }
        })
    }
}
