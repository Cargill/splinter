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

use splinter::error::InvalidStateError;
use splinter::service::FullyQualifiedServiceId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitEntry {
    service_id: FullyQualifiedServiceId,
    id: Option<i64>,
    value: String,
    decision: Option<ConsensusDecision>,
}

impl CommitEntry {
    /// Returns the service ID for the commit entry
    pub fn service_id(&self) -> &FullyQualifiedServiceId {
        &self.service_id
    }

    /// Returns the ID for the commit entry
    pub fn id(&self) -> &Option<i64> {
        &self.id
    }

    /// Returns the value for the commit entry
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the decision for the commit entry
    pub fn decision(&self) -> &Option<ConsensusDecision> {
        &self.decision
    }

    pub fn into_builder(self) -> CommitEntryBuilder {
        CommitEntryBuilder {
            service_id: Some(self.service_id),
            id: self.id,
            value: Some(self.value),
            decision: self.decision,
        }
    }
}

#[derive(Default, Clone)]
pub struct CommitEntryBuilder {
    service_id: Option<FullyQualifiedServiceId>,
    id: Option<i64>,
    value: Option<String>,
    decision: Option<ConsensusDecision>,
}

impl CommitEntryBuilder {
    /// Returns the service ID for the commit entry
    pub fn service_id(&self) -> Option<FullyQualifiedServiceId> {
        self.service_id.clone()
    }

    /// Returns the ID for the commit entry
    pub fn id(&self) -> Option<i64> {
        self.id
    }

    /// Returns the value for the commit entry
    pub fn value(&self) -> Option<String> {
        self.value.clone()
    }

    /// Returns the decision for the commit entry
    pub fn decision(&self) -> Option<ConsensusDecision> {
        self.decision.clone()
    }

    /// Sets the service ID
    ///
    /// # Arguments
    ///
    ///  * `service_id` - The service ID for commit entry
    pub fn with_service_id(mut self, service_id: &FullyQualifiedServiceId) -> CommitEntryBuilder {
        self.service_id = Some(service_id.clone());
        self
    }

    /// Sets the ID
    ///
    /// # Arguments
    ///
    ///  * `id` - The ID for commit entry
    pub fn with_id(mut self, id: i64) -> CommitEntryBuilder {
        self.id = Some(id);
        self
    }

    /// Sets the value
    ///
    /// # Arguments
    ///
    ///  * `value` - The value for commit entry that was being agreed upon
    pub fn with_value(mut self, value: &str) -> CommitEntryBuilder {
        self.value = Some(value.to_string());
        self
    }

    /// Sets the decision
    ///
    /// # Arguments
    ///
    ///  * `decision` - The decision for commit entry, either commit or abort
    pub fn with_decision(mut self, decision: &ConsensusDecision) -> CommitEntryBuilder {
        self.decision = Some(decision.clone());
        self
    }

    /// Builds the `CommitEntry`
    ///
    /// Returns an error if the service ID or value is not set
    pub fn build(self) -> Result<CommitEntry, InvalidStateError> {
        let service_id = self.service_id.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `service_id`".to_string(),
            )
        })?;

        let id = self.id;

        let value = self.value.ok_or_else(|| {
            InvalidStateError::with_message("unable to build, missing field: `value`".to_string())
        })?;

        Ok(CommitEntry {
            service_id,
            id,
            value,
            decision: self.decision,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConsensusDecision {
    Abort,
    Commit,
}

impl fmt::Display for ConsensusDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConsensusDecision::Abort => write!(f, "Decision: Abort"),
            ConsensusDecision::Commit => write!(f, "Decision: Commit"),
        }
    }
}
