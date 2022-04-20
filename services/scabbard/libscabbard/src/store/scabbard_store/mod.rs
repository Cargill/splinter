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

pub mod action;
pub mod commit;
pub mod context;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod diesel;
mod error;
mod factory;
pub mod service;
pub mod state;
pub mod two_phase;

use std::time::SystemTime;

pub(crate) use error::ScabbardStoreError;

use action::ScabbardConsensusAction;
use commit::CommitEntry;
use context::ScabbardContext;
use service::ScabbardService;
use splinter::service::FullyQualifiedServiceId;

#[cfg(feature = "postgres")]
pub use factory::{PgScabbardStoreFactory, PooledPgScabbardStoreFactory};
pub use factory::{PooledScabbardStoreFactory, ScabbardStoreFactory};
#[cfg(feature = "sqlite")]
pub use factory::{PooledSqliteScabbardStoreFactory, SqliteScabbardStoreFactory};

pub trait ScabbardStore {
    /// Add a new context
    ///
    /// # Arguments
    ///
    /// * `service_id` - The combined `CircuitId` and `ServiceId` of the service the context
    ///    belongs to
    /// * `context` - The `ScabbardContext` to be added to the database
    fn add_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ScabbardContext,
    ) -> Result<(), ScabbardStoreError>;
    /// Update an existing context
    ///
    /// # Arguments
    ///
    /// * `service_id` - The combined `CircuitId` and `ServiceId` of the service the context
    ///    belongs to
    ///    context
    /// * `context` - The `ScabbardContext` to be updated in the database
    fn update_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ScabbardContext,
    ) -> Result<(), ScabbardStoreError>;
    /// Add a 2 phase commit coordinator action
    ///
    /// # Arguments
    ///
    /// * `action` - The `ScabbardConsensusAction` to be added to the database
    /// * `service_id` - The combined `CircuitId` and `ServiceId` of the service the action
    ///    belongs to
    /// * `epoch` - The epoch that the given action belongs to
    fn add_consensus_action(
        &self,
        action: ScabbardConsensusAction,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<i64, ScabbardStoreError>;
    /// Update an existing 2 phase commit action
    ///
    /// # Arguments
    ///
    /// * `service_id` - The combined `CircuitId` and `ServiceId` of the service the action
    ///    belongs to
    /// * `epoch` - The epoch that the action belongs to
    /// * `action_id` - The ID of the action being updated
    /// * `executed_at` - The time that the action was executed
    fn update_consensus_action(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
        action_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError>;
    /// List all coordinator actions for a given service_id and epoch
    ///
    /// # Arguments
    ///
    /// * `service_id` - The combined `CircuitId` and `ServiceId` of the service for which actions
    ///    should be listed
    /// * `epoch` - The epoch for which actions should be listed
    fn list_consensus_actions(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<Vec<ScabbardConsensusAction>, ScabbardStoreError>;
    /// List ready services
    fn list_ready_services(&self) -> Result<Vec<FullyQualifiedServiceId>, ScabbardStoreError>;
    /// Add a new scabbard service
    ///
    /// # Arguments
    ///
    /// * `service` - The `ScabbardService` that is to be added to the database
    fn add_service(&self, service: ScabbardService) -> Result<(), ScabbardStoreError>;
    /// Add a new commit entry
    ///
    /// # Arguments
    ///
    /// * `commit_entry` - The `CommitEntry` that is to be added to the database
    fn add_commit_entry(&self, commit_entry: CommitEntry) -> Result<(), ScabbardStoreError>;
    /// Get the commit entry for the specified service_id and epoch
    ///
    /// # Arguments
    ///
    /// * `service_id` - The combined `CircuitId` and `ServiceId` of the service for which the last
    ///    commit entry should be retrieved
    fn get_last_commit_entry(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<CommitEntry>, ScabbardStoreError>;
    /// Update an existing commit entry
    ///
    /// # Arguments
    ///
    /// * `commit_entry` - The `CommitEntry` to be updated in the database
    fn update_commit_entry(&self, commit_entry: CommitEntry) -> Result<(), ScabbardStoreError>;
}
