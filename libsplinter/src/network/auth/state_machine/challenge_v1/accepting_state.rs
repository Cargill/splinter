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

use crate::network::auth::state_machine::{
    AuthorizationAcceptingAction, AuthorizationAcceptingState, AuthorizationActionError, Identity,
};

use super::ChallengeAuthorizationAcceptingAction;

#[derive(PartialEq, Debug, Clone)]
pub(crate) enum ChallengeAuthorizationAcceptingState {
    ChallengeConnecting,
    ReceivedAuthChallengeNonce,
    WaitingForAuthChallengeSubmitRequest,
    ReceivedAuthChallengeSubmitRequest(Identity),
}

impl ChallengeAuthorizationAcceptingState {
    /// Transitions from one authorization state to another
    ///
    /// Errors
    ///
    /// The errors are error messages that should be returned on the appropriate message
    pub(crate) fn next_accepting_state(
        &self,
        action: ChallengeAuthorizationAcceptingAction,
        cur_state: &mut ManagedAuthorizationState,
    ) -> Result<AuthorizationAcceptingState, AuthorizationActionError> {
        match &self {
            ChallengeAuthorizationAcceptingState::ChallengeConnecting => match action {
                ChallengeAuthorizationAcceptingAction::ReceiveAuthChallengeNonceRequest => {
                    let new_state = AuthorizationAcceptingState::Challenge(
                        ChallengeAuthorizationAcceptingState::ReceivedAuthChallengeNonce,
                    );
                    cur_state.accepting_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidAcceptingMessageOrder(
                    AuthorizationAcceptingState::Challenge(self.clone()),
                    AuthorizationAcceptingAction::Challenge(action),
                )),
            },
            ChallengeAuthorizationAcceptingState::ReceivedAuthChallengeNonce => match action {
                ChallengeAuthorizationAcceptingAction::SendAuthChallengeNonceResponse => {
                    let new_state = AuthorizationAcceptingState::Challenge(
                        ChallengeAuthorizationAcceptingState::WaitingForAuthChallengeSubmitRequest,
                    );
                    cur_state.accepting_state = new_state.clone();
                    Ok(new_state)
                }
                _ => Err(AuthorizationActionError::InvalidAcceptingMessageOrder(
                    AuthorizationAcceptingState::Challenge(self.clone()),
                    AuthorizationAcceptingAction::Challenge(action),
                )),
            },
            ChallengeAuthorizationAcceptingState::WaitingForAuthChallengeSubmitRequest => {
                match action {
                    ChallengeAuthorizationAcceptingAction::ReceiveAuthChallengeSubmitRequest(
                        identity,
                    ) => {
                        let new_state = AuthorizationAcceptingState::Challenge(
                        ChallengeAuthorizationAcceptingState::ReceivedAuthChallengeSubmitRequest(
                            identity,
                        ),
                    );
                        cur_state.accepting_state = new_state.clone();
                        Ok(new_state)
                    }
                    _ => Err(AuthorizationActionError::InvalidAcceptingMessageOrder(
                        AuthorizationAcceptingState::Challenge(self.clone()),
                        AuthorizationAcceptingAction::Challenge(action),
                    )),
                }
            }
            ChallengeAuthorizationAcceptingState::ReceivedAuthChallengeSubmitRequest(identity) => {
                match action {
                    ChallengeAuthorizationAcceptingAction::SendAuthChallengeSubmitResponse => {
                        let new_state = AuthorizationAcceptingState::Done(identity.clone());
                        cur_state.accepting_state = new_state.clone();
                        Ok(new_state)
                    }
                    _ => Err(AuthorizationActionError::InvalidAcceptingMessageOrder(
                        AuthorizationAcceptingState::Challenge(self.clone()),
                        AuthorizationAcceptingAction::Challenge(action),
                    )),
                }
            }
        }
    }
}

impl fmt::Display for ChallengeAuthorizationAcceptingState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            ChallengeAuthorizationAcceptingState::ChallengeConnecting => "ChallengeConnecting",
            ChallengeAuthorizationAcceptingState::ReceivedAuthChallengeNonce => {
                "ReceivedAuthChallengeNonce"
            }
            ChallengeAuthorizationAcceptingState::WaitingForAuthChallengeSubmitRequest => {
                "WaitingForAuthChallengeSubmitRequest"
            }
            ChallengeAuthorizationAcceptingState::ReceivedAuthChallengeSubmitRequest(_) => {
                "ReceivedAuthChallengeSubmitRequest"
            }
        })
    }
}
