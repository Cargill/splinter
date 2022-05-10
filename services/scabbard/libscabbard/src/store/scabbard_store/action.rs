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

use crate::store::scabbard_store::two_phase::Action;

#[derive(Debug, PartialEq, Clone)]
pub enum ConsensusAction {
    TwoPhaseCommit(Action),
}

// A scabbard consensus action that includes the action ID associated with the action
#[derive(Debug, PartialEq, Clone)]
pub enum IdentifiedConsensusAction {
    TwoPhaseCommit(i64, Action),
}

impl IdentifiedConsensusAction {
    pub fn deconstruct(self) -> (i64, ConsensusAction) {
        match self {
            Self::TwoPhaseCommit(id, action) => (id, ConsensusAction::TwoPhaseCommit(action)),
        }
    }
}
