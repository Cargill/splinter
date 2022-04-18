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

use std::convert::TryFrom;
use std::time::SystemTime;

use splinter::error::InvalidStateError;

use super::context::{CoordinatorState, ParticipantState};

#[derive(Clone, Debug, PartialEq)]
pub enum Scabbard2pcState {
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

impl TryFrom<Scabbard2pcState> for CoordinatorState {
    type Error = InvalidStateError;

    fn try_from(state: Scabbard2pcState) -> Result<Self, InvalidStateError> {
        match state {
            Scabbard2pcState::Abort => Ok(CoordinatorState::Abort),
            Scabbard2pcState::Commit => Ok(CoordinatorState::Commit),
            Scabbard2pcState::Voting { vote_timeout_start } => {
                Ok(CoordinatorState::Voting { vote_timeout_start })
            }
            Scabbard2pcState::WaitingForStart => Ok(CoordinatorState::WaitingForStart),
            Scabbard2pcState::WaitingForVote => Ok(CoordinatorState::WaitingForVote),
            _ => Err(InvalidStateError::with_message(format!(
                "invalid state for coordinator: {:?}",
                state
            ))),
        }
    }
}

impl TryFrom<Scabbard2pcState> for ParticipantState {
    type Error = InvalidStateError;

    fn try_from(state: Scabbard2pcState) -> Result<Self, InvalidStateError> {
        match state {
            Scabbard2pcState::Abort => Ok(ParticipantState::Abort),
            Scabbard2pcState::Commit => Ok(ParticipantState::Commit),
            Scabbard2pcState::Voted {
                vote,
                decision_timeout_start,
            } => Ok(ParticipantState::Voted {
                vote,
                decision_timeout_start,
            }),
            Scabbard2pcState::WaitingForVoteRequest => Ok(ParticipantState::WaitingForVoteRequest),
            Scabbard2pcState::WaitingForVote => Ok(ParticipantState::WaitingForVote),
            _ => Err(InvalidStateError::with_message(format!(
                "invalid state for participant: {:?}",
                state
            ))),
        }
    }
}

impl From<CoordinatorState> for Scabbard2pcState {
    fn from(state: CoordinatorState) -> Self {
        match state {
            CoordinatorState::Abort => Scabbard2pcState::Abort,
            CoordinatorState::Commit => Scabbard2pcState::Commit,
            CoordinatorState::Voting { vote_timeout_start } => {
                Scabbard2pcState::Voting { vote_timeout_start }
            }
            CoordinatorState::WaitingForStart => Scabbard2pcState::WaitingForStart,
            CoordinatorState::WaitingForVote => Scabbard2pcState::WaitingForVote,
        }
    }
}

impl From<ParticipantState> for Scabbard2pcState {
    fn from(state: ParticipantState) -> Self {
        match state {
            ParticipantState::Abort => Scabbard2pcState::Abort,
            ParticipantState::Commit => Scabbard2pcState::Commit,
            ParticipantState::Voted {
                vote,
                decision_timeout_start,
            } => Scabbard2pcState::Voted {
                vote,
                decision_timeout_start,
            },
            ParticipantState::WaitingForVoteRequest => Scabbard2pcState::WaitingForVoteRequest,
            ParticipantState::WaitingForVote => Scabbard2pcState::WaitingForVote,
        }
    }
}
