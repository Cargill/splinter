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

use super::ChallengeAuthorizationLocalAction;
use crate::network::auth::state_machine::{
    AuthorizationActionError, AuthorizationLocalAction, AuthorizationLocalState,
};

#[derive(PartialEq, Debug, Clone)]
pub(crate) enum ChallengeAuthorizationLocalState {
    ChallengeConnecting,
    WaitingForAuthChallengeNonceResponse,
    ReceivedAuthChallengeNonceResponse,
    WaitingForAuthChallengeSubmitResponse,
}

impl ChallengeAuthorizationLocalState {
    /// Transitions from one authorization state to another
    ///
    /// Errors
    ///
    /// The errors are error messages that should be returned on the appropriate message
    pub(crate) fn next_local_state(
        &self,
        action: ChallengeAuthorizationLocalAction,
        cur_state: &mut ManagedAuthorizationState,
    ) -> Result<AuthorizationLocalState, AuthorizationActionError> {
        match &self {
            ChallengeAuthorizationLocalState::ChallengeConnecting => match action {
                ChallengeAuthorizationLocalAction::SendAuthChallengeNonceRequest => {
                    let new_state = AuthorizationLocalState::Challenge(
                        ChallengeAuthorizationLocalState::WaitingForAuthChallengeNonceResponse,
                    );
                    cur_state.local_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidLocalMessageOrder(
                    AuthorizationLocalState::Challenge(self.clone()),
                    AuthorizationLocalAction::Challenge(action),
                )),
            },
            ChallengeAuthorizationLocalState::WaitingForAuthChallengeNonceResponse => {
                match action {
                    ChallengeAuthorizationLocalAction::ReceiveAuthChallengeNonceResponse => {
                        let new_state = AuthorizationLocalState::Challenge(
                            ChallengeAuthorizationLocalState::ReceivedAuthChallengeNonceResponse,
                        );
                        cur_state.local_state = new_state.clone();
                        Ok(new_state)
                    }
                    _ => Err(AuthorizationActionError::InvalidLocalMessageOrder(
                        AuthorizationLocalState::Challenge(self.clone()),
                        AuthorizationLocalAction::Challenge(action),
                    )),
                }
            }
            ChallengeAuthorizationLocalState::ReceivedAuthChallengeNonceResponse => match action {
                ChallengeAuthorizationLocalAction::SendAuthChallengeSubmitRequest => {
                    let new_state = AuthorizationLocalState::Challenge(
                        ChallengeAuthorizationLocalState::WaitingForAuthChallengeSubmitResponse,
                    );
                    cur_state.local_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidLocalMessageOrder(
                    AuthorizationLocalState::Challenge(self.clone()),
                    AuthorizationLocalAction::Challenge(action),
                )),
            },
            ChallengeAuthorizationLocalState::WaitingForAuthChallengeSubmitResponse => match action
            {
                ChallengeAuthorizationLocalAction::ReceiveAuthChallengeSubmitResponse(identity) => {
                    cur_state.local_authorization = Some(identity);
                    let new_state = AuthorizationLocalState::Authorized;
                    cur_state.local_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidLocalMessageOrder(
                    AuthorizationLocalState::Challenge(self.clone()),
                    AuthorizationLocalAction::Challenge(action),
                )),
            },
        }
    }
}

impl fmt::Display for ChallengeAuthorizationLocalState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            ChallengeAuthorizationLocalState::ChallengeConnecting => "ChallengeConnecting",
            ChallengeAuthorizationLocalState::WaitingForAuthChallengeNonceResponse => {
                "WaitingForAuthChallengeNonceResponse"
            }
            ChallengeAuthorizationLocalState::ReceivedAuthChallengeNonceResponse => {
                "ReceivedAuthChallengeNonceResponse"
            }
            ChallengeAuthorizationLocalState::WaitingForAuthChallengeSubmitResponse => {
                "WaitingForAuthChallengeSubmitResponse"
            }
        })
    }
}
