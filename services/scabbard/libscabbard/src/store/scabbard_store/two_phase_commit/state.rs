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

#[cfg(feature = "scabbardv3-consensus")]
use std::convert::TryFrom;
use std::time::SystemTime;

#[cfg(feature = "scabbardv3-consensus")]
use augrim::{error::InternalError, two_phase_commit::TwoPhaseCommitState};

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Abort,
    Commit,
    Voted {
        vote: bool,
        decision_timeout_start: SystemTime,
    },
    Voting {
        vote_timeout_start: SystemTime,
    },
    WaitingForStart,
    WaitingForVoteRequest,
    WaitingForVote,
}

#[cfg(feature = "scabbardv3-consensus")]
impl TryFrom<State> for TwoPhaseCommitState<SystemTime> {
    type Error = InternalError;

    fn try_from(state: State) -> Result<Self, Self::Error> {
        Ok(match state {
            State::Abort => Self::Abort,
            State::Commit => Self::Commit,
            State::Voted {
                vote,
                decision_timeout_start,
            } => Self::Voted {
                vote,
                decision_timeout_start,
            },
            State::Voting { vote_timeout_start } => Self::Voting { vote_timeout_start },
            State::WaitingForStart => Self::WaitingForStart,
            State::WaitingForVoteRequest => Self::WaitingForVoteRequest,
            State::WaitingForVote => Self::WaitingForVote,
        })
    }
}

#[cfg(feature = "scabbardv3-consensus")]
impl TryFrom<TwoPhaseCommitState<SystemTime>> for State {
    type Error = InternalError;

    fn try_from(state: TwoPhaseCommitState<SystemTime>) -> Result<Self, Self::Error> {
        Ok(match state {
            TwoPhaseCommitState::Abort => Self::Abort,
            TwoPhaseCommitState::Commit => Self::Commit,
            TwoPhaseCommitState::Voted {
                vote,
                decision_timeout_start,
            } => Self::Voted {
                vote,
                decision_timeout_start,
            },
            TwoPhaseCommitState::Voting { vote_timeout_start } => {
                Self::Voting { vote_timeout_start }
            }
            TwoPhaseCommitState::WaitingForStart => Self::WaitingForStart,
            TwoPhaseCommitState::WaitingForVoteRequest => Self::WaitingForVoteRequest,
            TwoPhaseCommitState::WaitingForVote => Self::WaitingForVote,
        })
    }
}
