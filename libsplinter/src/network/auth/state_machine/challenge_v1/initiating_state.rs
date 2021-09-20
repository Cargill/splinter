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

use super::ChallengeAuthorizationInitiatingAction;
use crate::network::auth::state_machine::{
    AuthorizationActionError, AuthorizationInitiatingAction, AuthorizationInitiatingState,
};

#[derive(PartialEq, Debug, Clone)]
pub(crate) enum ChallengeAuthorizationInitiatingState {
    ChallengeConnecting,
    WaitingForAuthChallengeNonceResponse,
    ReceivedAuthChallengeNonceResponse,
    WaitingForAuthChallengeSubmitResponse,
}

impl ChallengeAuthorizationInitiatingState {
    /// Transitions from one authorization state to another
    ///
    /// Errors
    ///
    /// The errors are error messages that should be returned on the appropriate message
    pub(crate) fn next_initiating_state(
        &self,
        action: ChallengeAuthorizationInitiatingAction,
        cur_state: &mut ManagedAuthorizationState,
    ) -> Result<AuthorizationInitiatingState, AuthorizationActionError> {
        match &self {
            ChallengeAuthorizationInitiatingState::ChallengeConnecting => match action {
                ChallengeAuthorizationInitiatingAction::SendAuthChallengeNonceRequest => {
                    let new_state = AuthorizationInitiatingState::Challenge(
                        ChallengeAuthorizationInitiatingState::WaitingForAuthChallengeNonceResponse,
                    );
                    cur_state.initiating_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidInitiatingMessageOrder(
                    AuthorizationInitiatingState::Challenge(self.clone()),
                    AuthorizationInitiatingAction::Challenge(action),
                )),
            },
            ChallengeAuthorizationInitiatingState::WaitingForAuthChallengeNonceResponse => {
                match action {
                    ChallengeAuthorizationInitiatingAction::ReceiveAuthChallengeNonceResponse => {
                        let new_state = AuthorizationInitiatingState::Challenge(
                            ChallengeAuthorizationInitiatingState::ReceivedAuthChallengeNonceResponse,
                        );
                        cur_state.initiating_state = new_state.clone();
                        Ok(new_state)
                    }
                    _ => Err(AuthorizationActionError::InvalidInitiatingMessageOrder(
                        AuthorizationInitiatingState::Challenge(self.clone()),
                        AuthorizationInitiatingAction::Challenge(action),
                    )),
                }
            }
            ChallengeAuthorizationInitiatingState::ReceivedAuthChallengeNonceResponse => {
                match action {
                    ChallengeAuthorizationInitiatingAction::SendAuthChallengeSubmitRequest => {
                        let new_state = AuthorizationInitiatingState::Challenge(
                        ChallengeAuthorizationInitiatingState::WaitingForAuthChallengeSubmitResponse,
                    );
                        cur_state.initiating_state = new_state.clone();
                        Ok(new_state)
                    }
                    _ => Err(AuthorizationActionError::InvalidInitiatingMessageOrder(
                        AuthorizationInitiatingState::Challenge(self.clone()),
                        AuthorizationInitiatingAction::Challenge(action),
                    )),
                }
            }
            ChallengeAuthorizationInitiatingState::WaitingForAuthChallengeSubmitResponse => {
                match action {
                    ChallengeAuthorizationInitiatingAction::ReceiveAuthChallengeSubmitResponse(
                        identity,
                    ) => {
                        cur_state.local_authorization = Some(identity);
                        let new_state = AuthorizationInitiatingState::Authorized;
                        cur_state.initiating_state = new_state.clone();
                        Ok(new_state)
                    }
                    _ => Err(AuthorizationActionError::InvalidInitiatingMessageOrder(
                        AuthorizationInitiatingState::Challenge(self.clone()),
                        AuthorizationInitiatingAction::Challenge(action),
                    )),
                }
            }
        }
    }
}

impl fmt::Display for ChallengeAuthorizationInitiatingState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            ChallengeAuthorizationInitiatingState::ChallengeConnecting => "ChallengeConnecting",
            ChallengeAuthorizationInitiatingState::WaitingForAuthChallengeNonceResponse => {
                "WaitingForAuthChallengeNonceResponse"
            }
            ChallengeAuthorizationInitiatingState::ReceivedAuthChallengeNonceResponse => {
                "ReceivedAuthChallengeNonceResponse"
            }
            ChallengeAuthorizationInitiatingState::WaitingForAuthChallengeSubmitResponse => {
                "WaitingForAuthChallengeSubmitResponse"
            }
        })
    }
}
