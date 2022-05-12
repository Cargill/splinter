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
use std::convert::{TryFrom, TryInto as _};
#[cfg(feature = "scabbardv3-consensus")]
use std::time::SystemTime;

#[cfg(feature = "scabbardv3-consensus")]
use augrim::{
    error::InternalError,
    two_phase_commit::{
        Participant as AugrimParticipant, TwoPhaseCommitContext, TwoPhaseCommitContextBuilder,
    },
};

use splinter::error::InvalidStateError;
use splinter::service::ServiceId;

use crate::store::scabbard_store::two_phase_commit::State;

#[cfg(feature = "scabbardv3-consensus")]
use crate::service::v3::ScabbardProcess;

#[derive(Debug, Clone, PartialEq)]
pub struct Context {
    coordinator: ServiceId,
    epoch: u64,
    last_commit_epoch: Option<u64>,
    this_process: ServiceId,
    participants: Vec<Participant>,
    state: State,
}

impl Context {
    pub fn coordinator(&self) -> &ServiceId {
        &self.coordinator
    }

    pub fn epoch(&self) -> &u64 {
        &self.epoch
    }

    pub fn last_commit_epoch(&self) -> Option<u64> {
        self.last_commit_epoch
    }

    pub fn this_process(&self) -> &ServiceId {
        &self.this_process
    }

    pub fn participants(&self) -> &[Participant] {
        &self.participants
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn into_builder(self) -> ContextBuilder {
        let mut builder = ContextBuilder::default()
            .with_coordinator(&self.coordinator)
            .with_epoch(self.epoch)
            .with_this_process(&self.this_process)
            .with_state(self.state)
            .with_participants(self.participants);

        if let Some(last_commit_epoch) = self.last_commit_epoch {
            builder = builder.with_last_commit_epoch(last_commit_epoch);
        }

        builder
    }
}

#[derive(Default, Clone)]
pub struct ContextBuilder {
    coordinator: Option<ServiceId>,
    epoch: Option<u64>,
    last_commit_epoch: Option<u64>,
    participants: Option<Vec<Participant>>,
    state: Option<State>,
    this_process: Option<ServiceId>,
}

impl ContextBuilder {
    pub fn new() -> Self {
        Self::default()
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

    pub fn with_state(mut self, state: State) -> ContextBuilder {
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

        let participants = self.participants.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `participants`".to_string(),
            )
        })?;

        Ok(Context {
            coordinator,
            epoch,
            last_commit_epoch: self.last_commit_epoch,
            this_process,
            participants,
            state,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Participant {
    pub process: ServiceId,
    pub vote: Option<bool>,
}

#[cfg(feature = "scabbardv3-consensus")]
impl TryFrom<TwoPhaseCommitContext<ScabbardProcess, SystemTime>> for Context {
    type Error = InternalError;

    fn try_from(
        ctx: TwoPhaseCommitContext<ScabbardProcess, SystemTime>,
    ) -> Result<Self, Self::Error> {
        let mut builder = ContextBuilder::default()
            .with_epoch(*ctx.epoch())
            .with_coordinator(&ctx.coordinator().clone().into())
            .with_this_process(&ctx.this_process().clone().into())
            .with_state(ctx.state().try_into()?);

        if let Some(last_commit_epoch) = ctx.last_commit_epoch() {
            builder = builder.with_last_commit_epoch(*last_commit_epoch);
        }

        if let Some(participants) = ctx.participants() {
            builder = builder.with_participants(
                participants
                    .iter()
                    .cloned()
                    .map(|p| Participant {
                        process: p.process.into(),
                        vote: p.vote,
                    })
                    .collect(),
            );
        }

        if let Some(participant_processes) = ctx.participant_processes() {
            builder = builder.with_participants(
                participant_processes
                    .iter()
                    .cloned()
                    .map(|p| Participant {
                        process: p.into(),
                        vote: None,
                    })
                    .collect(),
            );
        }

        builder
            .build()
            .map_err(|e| InternalError::from_source(Box::new(e)))
    }
}

#[cfg(feature = "scabbardv3-consensus")]
impl TryFrom<Context> for TwoPhaseCommitContext<ScabbardProcess, SystemTime> {
    type Error = InternalError;

    fn try_from(ctx: Context) -> Result<Self, Self::Error> {
        let Context {
            coordinator,
            epoch,
            last_commit_epoch,
            this_process,
            participants,
            state,
            ..
        } = ctx;

        let mut builder = TwoPhaseCommitContextBuilder::new();

        if coordinator == this_process {
            builder = builder.with_participants(
                participants
                    .into_iter()
                    .map(|p| AugrimParticipant {
                        process: p.process.into(),
                        vote: p.vote,
                    })
                    .collect(),
            );
        } else {
            builder = builder.with_participant_processes(
                participants.into_iter().map(|p| p.process.into()).collect(),
            );
        }

        builder = builder
            .with_this_process(this_process.into())
            .with_coordinator(coordinator.into())
            .with_epoch(epoch)
            .with_state(state.try_into()?);

        if let Some(last_commit_epoch) = last_commit_epoch {
            builder = builder.with_last_commit_epoch(last_commit_epoch);
        }

        builder
            .build()
            .map_err(|e| InternalError::from_source(Box::new(e)))
    }
}
