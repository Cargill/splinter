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

use std::convert::{TryFrom, TryInto};
use std::time::SystemTime;

use splinter::error::InvalidStateError;
use splinter::service::ServiceId;

use crate::store::scabbard_store::state::Scabbard2pcState;

#[derive(Debug, Clone, PartialEq)]
pub enum ScabbardContext {
    Scabbard2pcContext(Context),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Context {
    alarm: Option<SystemTime>,
    coordinator: ServiceId,
    epoch: u64,
    last_commit_epoch: Option<u64>,
    role_context: TwoPhaseCommitRoleContext,
    this_process: ServiceId,
}

impl Context {
    pub fn alarm(&self) -> Option<SystemTime> {
        self.alarm
    }

    pub fn coordinator(&self) -> &ServiceId {
        &self.coordinator
    }

    pub fn epoch(&self) -> &u64 {
        &self.epoch
    }

    pub fn last_commit_epoch(&self) -> Option<u64> {
        self.last_commit_epoch
    }

    pub fn role_context(&self) -> &TwoPhaseCommitRoleContext {
        &self.role_context
    }

    pub fn this_process(&self) -> &ServiceId {
        &self.this_process
    }
}

#[derive(Default, Clone)]
pub struct ContextBuilder {
    alarm: Option<SystemTime>,
    coordinator: Option<ServiceId>,
    epoch: Option<u64>,
    last_commit_epoch: Option<u64>,
    participants: Option<Vec<Participant>>,
    participant_processes: Option<Vec<ServiceId>>,
    state: Option<Scabbard2pcState>,
    this_process: Option<ServiceId>,
}

impl ContextBuilder {
    pub fn with_alarm(mut self, alarm: SystemTime) -> ContextBuilder {
        self.alarm = Some(alarm);
        self
    }

    pub fn with_coordinator(mut self, coordinator: &ServiceId) -> ContextBuilder {
        self.coordinator = Some(coordinator.clone());
        self
    }

    pub fn with_epoch(mut self, epoch: u64) -> ContextBuilder {
        self.epoch = Some(epoch);
        self
    }

    pub fn with_last_commit_epoch(mut self, last_commit_epoch: u64) -> ContextBuilder {
        self.last_commit_epoch = Some(last_commit_epoch);
        self
    }

    pub fn with_participants(mut self, participants: Vec<Participant>) -> ContextBuilder {
        self.participants = Some(participants);
        self
    }

    pub fn with_participant_processes(
        mut self,
        participant_processes: Vec<ServiceId>,
    ) -> ContextBuilder {
        self.participant_processes = Some(participant_processes);
        self
    }

    pub fn with_state(mut self, state: Scabbard2pcState) -> ContextBuilder {
        self.state = Some(state);
        self
    }

    pub fn with_this_process(mut self, this_process: &ServiceId) -> ContextBuilder {
        self.this_process = Some(this_process.clone());
        self
    }

    pub fn build(self) -> Result<Context, InvalidStateError> {
        let coordinator = self.coordinator.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `coordinator`".to_string(),
            )
        })?;

        let epoch = self.epoch.ok_or_else(|| {
            InvalidStateError::with_message("unable to build, missing field: `epoch`".to_string())
        })?;

        let state = self.state.ok_or_else(|| {
            InvalidStateError::with_message("unable to build, missing field: `state`".to_string())
        })?;

        let this_process = self.this_process.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `this_process`".to_string(),
            )
        })?;

        let role_context = match (self.participants, self.participant_processes) {
            (Some(participants), None) => Ok(TwoPhaseCommitRoleContext {
                inner: InnerContext::Coordinator(CoordinatorContext {
                    participants,
                    state: state.try_into()?,
                }),
            }),
            (None, Some(participant_processes)) => Ok(TwoPhaseCommitRoleContext {
                inner: InnerContext::Participant(ParticipantContext {
                    participant_processes,
                    state: state.try_into()?,
                }),
            }),
            (Some(_), Some(_)) => Err(InvalidStateError::with_message(
                "participant and participant_processes fields are mutually exclusive".into(),
            )),
            (None, None) => Err(InvalidStateError::with_message(
                "exactly one of participant or particpant_processes fields required".into(),
            )),
        }?;

        Ok(Context {
            alarm: self.alarm,
            coordinator,
            epoch,
            last_commit_epoch: self.last_commit_epoch,
            role_context,
            this_process,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TwoPhaseCommitRoleContext {
    inner: InnerContext,
}

impl TryFrom<TwoPhaseCommitRoleContext> for CoordinatorContext {
    type Error = InvalidStateError;

    fn try_from(context: TwoPhaseCommitRoleContext) -> Result<Self, Self::Error> {
        match context.inner {
            InnerContext::Coordinator(c) => Ok(c),
            InnerContext::Participant(_) => Err(InvalidStateError::with_message(
                "unable to convert TwoPhaseCommitRoleContext to CoordinatorContext \
                because inner context type is Participant"
                    .into(),
            )),
        }
    }
}

impl TryFrom<TwoPhaseCommitRoleContext> for ParticipantContext {
    type Error = InvalidStateError;

    fn try_from(context: TwoPhaseCommitRoleContext) -> Result<Self, Self::Error> {
        match context.inner {
            InnerContext::Participant(c) => Ok(c),
            InnerContext::Coordinator(_) => Err(InvalidStateError::with_message(
                "unable to convert TwoPhaseCommitRoleContext to ParticipantContext \
                because inner context type is Coordinator"
                    .into(),
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum InnerContext {
    Coordinator(CoordinatorContext),
    Participant(ParticipantContext),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CoordinatorContext {
    pub participants: Vec<Participant>,
    pub state: CoordinatorState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Participant {
    pub process: ServiceId,
    pub vote: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CoordinatorState {
    Abort,
    Commit,
    Voting { vote_timeout_start: SystemTime },
    WaitingForStart,
    WaitingForVote,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParticipantContext {
    pub participant_processes: Vec<ServiceId>,
    pub state: ParticipantState,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ParticipantState {
    Abort,
    Commit,
    Voted {
        vote: bool,
        decision_timeout_start: SystemTime,
    },
    WaitingForVoteRequest,
    WaitingForVote,
}
