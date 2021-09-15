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

use crate::network::auth::state_machine::Identity;

/// The state transitions that can be applied on a connection during authorization.
#[derive(PartialEq, Debug)]
pub(crate) enum ChallengeAuthorizationLocalAction {
    SendAuthChallengeNonceRequest,
    ReceiveAuthChallengeNonceResponse,
    SendAuthChallengeSubmitRequest,
    ReceiveAuthChallengeSubmitResponse(Identity),
}

impl fmt::Display for ChallengeAuthorizationLocalAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ChallengeAuthorizationLocalAction::SendAuthChallengeNonceRequest => {
                f.write_str("SendAuthChallengeNonceRequest")
            }
            ChallengeAuthorizationLocalAction::ReceiveAuthChallengeNonceResponse => {
                f.write_str("ReceiveAuthChallengeNonceResponse")
            }
            ChallengeAuthorizationLocalAction::SendAuthChallengeSubmitRequest => {
                f.write_str("SendAuthChallengeSubmitRequest")
            }
            ChallengeAuthorizationLocalAction::ReceiveAuthChallengeSubmitResponse(_) => {
                f.write_str("ReceiveAuthChallengeSubmitResponse")
            }
        }
    }
}
