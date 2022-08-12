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
use augrim::{error::InternalError, two_phase_commit::TwoPhaseCommitEvent};

#[cfg(feature = "scabbardv3-consensus")]
use crate::service::v3::{ScabbardProcess, ScabbardValue};
use crate::store::scabbard_store::two_phase_commit::Event;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ConsensusEvent {
    TwoPhaseCommit(Event),
}

impl ConsensusEvent {
    pub fn algorithm_name(&self) -> &str {
        match self {
            Self::TwoPhaseCommit(_) => "two-phase-commit",
        }
    }
}

#[cfg(feature = "scabbardv3-consensus")]
impl TryFrom<ConsensusEvent> for TwoPhaseCommitEvent<ScabbardProcess, ScabbardValue> {
    type Error = InternalError;

    fn try_from(event: ConsensusEvent) -> Result<Self, Self::Error> {
        match event {
            ConsensusEvent::TwoPhaseCommit(event) => event.try_into(),
        }
    }
}
