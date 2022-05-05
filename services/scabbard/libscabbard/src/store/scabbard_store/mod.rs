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
pub mod alarm;
mod boxed;
pub mod commit;
pub mod context;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod diesel;
mod error;
pub mod event;
mod factory;
pub mod service;
pub mod two_phase_commit;

use std::time::SystemTime;

pub(crate) use error::ScabbardStoreError;

use action::{ConsensusAction, IdentifiedConsensusAction};
use alarm::AlarmType;
use commit::CommitEntry;
use context::ConsensusContext;
use event::{ConsensusEvent, IdentifiedConsensusEvent};
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
    /// * `context` - The `ConsensusContext` to be added to the database
    fn add_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ConsensusContext,
    ) -> Result<(), ScabbardStoreError>;

    /// Update an existing context
    ///
    /// # Arguments
    ///
    /// * `service_id` - The combined `CircuitId` and `ServiceId` of the service the context
    ///    belongs to
    ///    context
    /// * `context` - The `ConsensusContext` to be updated in the database
    fn update_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ConsensusContext,
    ) -> Result<(), ScabbardStoreError>;

    /// Add a 2 phase commit coordinator action
    ///
    /// # Arguments
    ///
    /// * `action` - The `ConsensusAction` to be added to the database
    /// * `service_id` - The combined `CircuitId` and `ServiceId` of the service the action
    ///    belongs to
    /// * `epoch` - The epoch that the given action belongs to
    fn add_consensus_action(
        &self,
        action: ConsensusAction,
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

    /// List all pending consensus actions for a given service_id and epoch
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
    ) -> Result<Vec<IdentifiedConsensusAction>, ScabbardStoreError>;

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

    /// Update an existing scabbard service
    ///
    /// # Arguments
    ///
    /// * `service` - The `ScabbardService` to be updated
    fn update_service(&self, service: ScabbardService) -> Result<(), ScabbardStoreError>;

    /// Returns a scabbard service
    ///
    /// # Arguments
    ///
    /// * `service_id` - The fully qualified service id for the `ScabbardService` to be returned
    fn get_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<ScabbardService>, ScabbardStoreError>;

    /// Add a new consensus event
    ///
    /// # Arguments
    ///
    /// * `service_id` - The combined `CircuitId` and `ServiceId` of the service the event
    ///    belongs to
    /// * `epoch` - The epoch that the event belongs to
    /// * `event` - The `ConsensusEvent` to be added
    fn add_consensus_event(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
        event: ConsensusEvent,
    ) -> Result<i64, ScabbardStoreError>;

    /// Update an existing consensus event
    ///
    /// # Arguments
    ///
    /// * `service_id` - The combined `CircuitId` and `ServiceId` of the service the event
    ///    belongs to
    /// * `epoch` - The epoch that the event belongs to
    /// * `event_id` - The ID of the event to be updated
    /// * `executed_at` - The time that the event was executed
    fn update_consensus_event(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
        event_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError>;

    /// List all pending consensus events for a given service_id and epoch
    ///
    /// # Arguments
    ///
    /// * `service_id` - The combined `CircuitId` and `ServiceId` of the service for which events
    ///    should be listed
    /// * `epoch` - The epoch for which events should be listed
    fn list_consensus_events(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<Vec<IdentifiedConsensusEvent>, ScabbardStoreError>;

    /// Get the current context for a given service
    ///
    /// # Arguments
    ///
    /// * `service_id` - The combined `CircuitId` and `ServiceId` of the service for which the
    ///    current context should be retrieved
    fn get_current_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<ConsensusContext>, ScabbardStoreError>;

    /// Removes a scabbard service and all of its associated state
    ///
    /// # Arguments
    ///
    /// * `service_id` - The fully qualified service id for the `ScabbardService` to be removed
    fn remove_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<(), ScabbardStoreError>;

    /// Set a scabbard alarm for a given service
    ///
    /// # Arguments
    ///
    /// * `service_id` - The fully qualified service id for the `ScabbardService` that the alarm
    ///   will be set for
    /// * `alarm` - The time that the alarm will go off
    /// * `alarm_type` - The type of alarm being set
    fn set_alarm(
        &self,
        service_id: &FullyQualifiedServiceId,
        alarm_type: &AlarmType,
        alarm: SystemTime,
    ) -> Result<(), ScabbardStoreError>;
}
