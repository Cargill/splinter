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

use std::time::SystemTime;

use splinter::service::FullyQualifiedServiceId;

use crate::store::scabbard_store::ScabbardStoreError;
use crate::store::scabbard_store::{
    AlarmType, CommitEntry, ConsensusAction, ConsensusContext, ConsensusEvent, Identified,
    ScabbardService,
};

use super::ScabbardStore;

impl ScabbardStore for Box<dyn ScabbardStore> {
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
    ) -> Result<(), ScabbardStoreError> {
        (&**self).add_consensus_context(service_id, context)
    }

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
    ) -> Result<(), ScabbardStoreError> {
        (&**self).update_consensus_context(service_id, context)
    }

    /// Add a 2 phase commit coordinator action
    ///
    /// # Arguments
    ///
    /// * `action` - The `ConsensusAction` to be added to the database
    /// * `service_id` - The combined `CircuitId` and `ServiceId` of the service the action
    ///    belongs to
    fn add_consensus_action(
        &self,
        action: ConsensusAction,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<i64, ScabbardStoreError> {
        (&**self).add_consensus_action(action, service_id)
    }

    /// Update an existing 2 phase commit action
    ///
    /// # Arguments
    ///
    /// * `service_id` - The combined `CircuitId` and `ServiceId` of the service the action
    ///    belongs to
    /// * `action_id` - The ID of the action being updated
    /// * `executed_at` - The time that the action was executed
    fn update_consensus_action(
        &self,
        service_id: &FullyQualifiedServiceId,
        action_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        (&**self).update_consensus_action(service_id, action_id, executed_at)
    }

    /// List all coordinator actions for a given service_id
    ///
    /// # Arguments
    ///
    /// * `service_id` - The combined `CircuitId` and `ServiceId` of the service for which actions
    ///    should be listed
    fn list_consensus_actions(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Vec<Identified<ConsensusAction>>, ScabbardStoreError> {
        (&**self).list_consensus_actions(service_id)
    }

    /// List ready services
    fn list_ready_services(&self) -> Result<Vec<FullyQualifiedServiceId>, ScabbardStoreError> {
        (&**self).list_ready_services()
    }

    /// Add a new scabbard service
    ///
    /// # Arguments
    ///
    /// * `service` - The `ScabbardService` that is to be added to the database
    fn add_service(&self, service: ScabbardService) -> Result<(), ScabbardStoreError> {
        (&**self).add_service(service)
    }

    /// Add a new commit entry
    ///
    /// # Arguments
    ///
    /// * `commit_entry` - The `CommitEntry` that is to be added to the database
    fn add_commit_entry(&self, commit_entry: CommitEntry) -> Result<(), ScabbardStoreError> {
        (&**self).add_commit_entry(commit_entry)
    }

    /// Get the commit entry for the specified service_id and epoch
    ///
    /// # Arguments
    ///
    /// * `service_id` - The combined `CircuitId` and `ServiceId` of the service for which the last
    ///    commit entry should be retrieved
    fn get_last_commit_entry(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<CommitEntry>, ScabbardStoreError> {
        (&**self).get_last_commit_entry(service_id)
    }

    /// Update an existing commit entry
    ///
    /// # Arguments
    ///
    /// * `commit_entry` - The `CommitEntry` to be updated in the database
    fn update_commit_entry(&self, commit_entry: CommitEntry) -> Result<(), ScabbardStoreError> {
        (&**self).update_commit_entry(commit_entry)
    }

    /// Update an existing scabbard service
    ///
    /// # Arguments
    ///
    /// * `service` - The `ScabbardService` to be updated
    fn update_service(&self, service: ScabbardService) -> Result<(), ScabbardStoreError> {
        (&**self).update_service(service)
    }

    /// Returns a scabbard service
    ///
    /// # Arguments
    ///
    /// * `service_id` - The fully qualified service id for the `ScabbardService` to be returned
    fn get_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<ScabbardService>, ScabbardStoreError> {
        (&**self).get_service(service_id)
    }

    /// Add a new consensus event
    ///
    /// # Arguments
    ///
    /// * `service_id` - The combined `CircuitId` and `ServiceId` of the service the event
    ///    belongs to
    /// * `event` - The `ConsensusEvent` to be added
    fn add_consensus_event(
        &self,
        service_id: &FullyQualifiedServiceId,
        event: ConsensusEvent,
    ) -> Result<i64, ScabbardStoreError> {
        (&**self).add_consensus_event(service_id, event)
    }

    /// Update an existing consensus event
    ///
    /// # Arguments
    ///
    /// * `service_id` - The combined `CircuitId` and `ServiceId` of the service the event
    ///    belongs to
    /// * `event_id` - The ID of the event to be updated
    /// * `executed_at` - The time that the event was executed
    fn update_consensus_event(
        &self,
        service_id: &FullyQualifiedServiceId,
        event_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        (&**self).update_consensus_event(service_id, event_id, executed_at)
    }

    /// List all consensus events for a given service_id and epoch
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
    ) -> Result<Vec<Identified<ConsensusEvent>>, ScabbardStoreError> {
        (&**self).list_consensus_events(service_id, epoch)
    }

    /// Get the current context for a given service
    ///
    /// # Arguments
    ///
    /// * `service_id` - The combined `CircuitId` and `ServiceId` of the service for which the
    ///    current context should be retrieved
    fn get_current_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<ConsensusContext>, ScabbardStoreError> {
        (&**self).get_current_consensus_context(service_id)
    }

    /// Removes a scabbard service and all of its associated state
    ///
    /// # Arguments
    ///
    /// * `service_id` - The fully qualified service id for the `ScabbardService` to be removed
    fn remove_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<(), ScabbardStoreError> {
        (&**self).remove_service(service_id)
    }

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
    ) -> Result<(), ScabbardStoreError> {
        (&**self).set_alarm(service_id, alarm_type, alarm)
    }

    /// Unset a scabbard alarm of a specified type for a given service
    ///
    /// # Arguments
    ///
    /// * `service_id` - The fully qualified service id for the `ScabbardService` that the alarm
    ///   will be unset for
    /// * `alarm_type` - The type of alarm being unset
    fn unset_alarm(
        &self,
        service_id: &FullyQualifiedServiceId,
        alarm_type: &AlarmType,
    ) -> Result<(), ScabbardStoreError> {
        (&**self).unset_alarm(service_id, alarm_type)
    }

    /// Get the scabbard alarm of a specified type for the given service
    ///
    /// # Arguments
    ///
    /// * `service_id` - The fully qualified service id for the `ScabbardService` to retrieve the
    ///    alarm for
    /// * `alarm_type` - The type of alarm to retrieve
    fn get_alarm(
        &self,
        service_id: &FullyQualifiedServiceId,
        alarm_type: &AlarmType,
    ) -> Result<Option<SystemTime>, ScabbardStoreError> {
        (&**self).get_alarm(service_id, alarm_type)
    }
}
