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

mod models;
mod operations;
mod schema;

use std::sync::{Arc, RwLock};
use std::time::SystemTime;

#[cfg(feature = "postgres")]
use diesel::pg::PgConnection;
#[cfg(feature = "sqlite")]
use diesel::sqlite::SqliteConnection;
use diesel::{
    connection::AnsiTransactionManager,
    r2d2::{ConnectionManager, Pool},
    Connection,
};

use crate::store::pool::ConnectionPool;
use crate::store::scabbard_store::ScabbardStoreError;
use crate::store::scabbard_store::{
    AlarmType, CommitEntry, ConsensusAction, ConsensusContext, ConsensusEvent, Identified,
    ScabbardService,
};

use super::ScabbardStore;

use operations::add_commit_entry::AddCommitEntryOperation as _;
use operations::add_consensus_action::AddActionOperation as _;
use operations::add_consensus_context::AddContextOperation as _;
use operations::add_consensus_event::AddEventOperation as _;
use operations::add_service::AddServiceOperation as _;
use operations::get_alarm::GetAlarmOperation as _;
use operations::get_current_consensus_context::GetCurrentContextAction as _;
use operations::get_last_commit_entry::GetLastCommitEntryOperation as _;
use operations::get_service::GetServiceOperation as _;
use operations::list_consensus_actions::ListActionsOperation as _;
use operations::list_consensus_events::ListEventsOperation as _;
use operations::list_ready_services::ListReadyServicesOperation as _;
use operations::remove_service::RemoveServiceOperation as _;
use operations::set_alarm::SetAlarmOperation as _;
use operations::unset_alarm::UnsetAlarmOperation as _;
use operations::update_commit_entry::UpdateCommitEntryOperation as _;
use operations::update_consensus_action::UpdateActionOperation as _;
use operations::update_consensus_context::UpdateContextAction as _;
use operations::update_consensus_event::UpdateEventOperation as _;
use operations::update_service::UpdateServiceAction as _;
use operations::ScabbardStoreOperations;

use splinter::service::FullyQualifiedServiceId;

pub struct DieselScabbardStore<C: Connection + 'static> {
    pool: ConnectionPool<C>,
}

impl<C: Connection> DieselScabbardStore<C> {
    pub fn new(connection_pool: Pool<ConnectionManager<C>>) -> Self {
        Self {
            pool: connection_pool.into(),
        }
    }

    pub fn new_with_write_exclusivity(
        connection_pool: Arc<RwLock<Pool<ConnectionManager<C>>>>,
    ) -> Self {
        Self {
            pool: connection_pool.into(),
        }
    }
}

#[cfg(feature = "sqlite")]
impl ScabbardStore for DieselScabbardStore<SqliteConnection> {
    /// Add a new context
    fn add_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ConsensusContext,
    ) -> Result<(), ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).add_consensus_context(service_id, context)
        })
    }
    /// Update an existing context
    fn update_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ConsensusContext,
    ) -> Result<(), ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).update_consensus_context(service_id, context)
        })
    }
    /// Add a 2 phase commit coordinator action
    fn add_consensus_action(
        &self,
        action: ConsensusAction,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<i64, ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).add_consensus_action(action, service_id)
        })
    }
    /// Update an existing 2 phase commit action
    fn update_consensus_action(
        &self,
        service_id: &FullyQualifiedServiceId,
        action_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).update_consensus_action(
                service_id,
                action_id,
                executed_at,
            )
        })
    }
    /// List all coordinator actions for a given service_id
    fn list_consensus_actions(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Vec<Identified<ConsensusAction>>, ScabbardStoreError> {
        self.pool.execute_read(|conn| {
            ScabbardStoreOperations::new(conn).list_consensus_actions(service_id)
        })
    }
    /// List ready services
    fn list_ready_services(&self) -> Result<Vec<FullyQualifiedServiceId>, ScabbardStoreError> {
        self.pool
            .execute_read(|conn| ScabbardStoreOperations::new(conn).list_ready_services())
    }
    /// Add a new scabbard service
    fn add_service(&self, service: ScabbardService) -> Result<(), ScabbardStoreError> {
        self.pool
            .execute_write(|conn| ScabbardStoreOperations::new(conn).add_service(service))
    }
    /// Add a new commit entry
    fn add_commit_entry(&self, commit_entry: CommitEntry) -> Result<(), ScabbardStoreError> {
        self.pool
            .execute_write(|conn| ScabbardStoreOperations::new(conn).add_commit_entry(commit_entry))
    }
    /// Get the commit entry for the specified service_id
    fn get_last_commit_entry(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<CommitEntry>, ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).get_last_commit_entry(service_id)
        })
    }
    /// Update an existing commit entry
    fn update_commit_entry(&self, commit_entry: CommitEntry) -> Result<(), ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).update_commit_entry(commit_entry)
        })
    }
    /// Update an existing scabbard service
    fn update_service(&self, service: ScabbardService) -> Result<(), ScabbardStoreError> {
        self.pool
            .execute_write(|conn| ScabbardStoreOperations::new(conn).update_service(service))
    }
    /// Get a service
    fn get_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<ScabbardService>, ScabbardStoreError> {
        self.pool
            .execute_read(|conn| ScabbardStoreOperations::new(conn).get_service(service_id))
    }
    /// Add a new consensus event
    fn add_consensus_event(
        &self,
        service_id: &FullyQualifiedServiceId,
        event: ConsensusEvent,
    ) -> Result<i64, ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).add_consensus_event(service_id, event)
        })
    }
    /// Update an existing consensus event
    fn update_consensus_event(
        &self,
        service_id: &FullyQualifiedServiceId,
        event_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).update_consensus_event(
                service_id,
                event_id,
                executed_at,
            )
        })
    }
    /// List all consensus events for a given service_id
    fn list_consensus_events(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Vec<Identified<ConsensusEvent>>, ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).list_consensus_events(service_id)
        })
    }
    /// Get the current context for a given service
    fn get_current_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<ConsensusContext>, ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).get_current_consensus_context(service_id)
        })
    }

    /// Remove existing service
    fn remove_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<(), ScabbardStoreError> {
        self.pool
            .execute_write(|conn| ScabbardStoreOperations::new(conn).remove_service(service_id))
    }

    /// Set a scabbard alarm
    fn set_alarm(
        &self,
        service_id: &FullyQualifiedServiceId,
        alarm_type: &AlarmType,
        alarm: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).set_alarm(service_id, alarm_type, alarm)
        })
    }

    /// Unset a scabbard alarm
    fn unset_alarm(
        &self,
        service_id: &FullyQualifiedServiceId,
        alarm_type: &AlarmType,
    ) -> Result<(), ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).unset_alarm(service_id, alarm_type)
        })
    }

    /// Get the scabbard alarm of a specified type for the given service
    fn get_alarm(
        &self,
        service_id: &FullyQualifiedServiceId,
        alarm_type: &AlarmType,
    ) -> Result<Option<SystemTime>, ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).get_alarm(service_id, alarm_type)
        })
    }
}

#[cfg(feature = "postgres")]
impl ScabbardStore for DieselScabbardStore<PgConnection> {
    /// Add a new context
    fn add_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ConsensusContext,
    ) -> Result<(), ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).add_consensus_context(service_id, context)
        })
    }
    /// Update an existing context
    fn update_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ConsensusContext,
    ) -> Result<(), ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).update_consensus_context(service_id, context)
        })
    }
    /// Add a 2 phase commit coordinator action
    fn add_consensus_action(
        &self,
        action: ConsensusAction,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<i64, ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).add_consensus_action(action, service_id)
        })
    }
    /// Update an existing 2 phase commit action
    fn update_consensus_action(
        &self,
        service_id: &FullyQualifiedServiceId,
        action_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).update_consensus_action(
                service_id,
                action_id,
                executed_at,
            )
        })
    }
    /// List all coordinator actions for a given service_id
    fn list_consensus_actions(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Vec<Identified<ConsensusAction>>, ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).list_consensus_actions(service_id)
        })
    }
    /// List ready services
    fn list_ready_services(&self) -> Result<Vec<FullyQualifiedServiceId>, ScabbardStoreError> {
        self.pool
            .execute_write(|conn| ScabbardStoreOperations::new(conn).list_ready_services())
    }
    /// Add a new scabbard service
    fn add_service(&self, service: ScabbardService) -> Result<(), ScabbardStoreError> {
        self.pool
            .execute_write(|conn| ScabbardStoreOperations::new(conn).add_service(service))
    }
    /// Add a new commit entry
    fn add_commit_entry(&self, commit_entry: CommitEntry) -> Result<(), ScabbardStoreError> {
        self.pool
            .execute_write(|conn| ScabbardStoreOperations::new(conn).add_commit_entry(commit_entry))
    }
    /// Get the commit entry for the specified service_id
    fn get_last_commit_entry(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<CommitEntry>, ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).get_last_commit_entry(service_id)
        })
    }
    /// Update an existing commit entry
    fn update_commit_entry(&self, commit_entry: CommitEntry) -> Result<(), ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).update_commit_entry(commit_entry)
        })
    }
    /// Update an existing scabbard service
    fn update_service(&self, service: ScabbardService) -> Result<(), ScabbardStoreError> {
        self.pool
            .execute_write(|conn| ScabbardStoreOperations::new(conn).update_service(service))
    }
    /// Get a service
    fn get_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<ScabbardService>, ScabbardStoreError> {
        self.pool
            .execute_read(|conn| ScabbardStoreOperations::new(conn).get_service(service_id))
    }
    /// Add a new consensus event
    fn add_consensus_event(
        &self,
        service_id: &FullyQualifiedServiceId,
        event: ConsensusEvent,
    ) -> Result<i64, ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).add_consensus_event(service_id, event)
        })
    }
    /// Update an existing consensus event
    fn update_consensus_event(
        &self,
        service_id: &FullyQualifiedServiceId,
        event_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).update_consensus_event(
                service_id,
                event_id,
                executed_at,
            )
        })
    }
    /// List all consensus events for a given service_id
    fn list_consensus_events(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Vec<Identified<ConsensusEvent>>, ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).list_consensus_events(service_id)
        })
    }
    /// Get the current context for a given service
    fn get_current_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<ConsensusContext>, ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).get_current_consensus_context(service_id)
        })
    }

    /// Remove existing service
    fn remove_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<(), ScabbardStoreError> {
        self.pool
            .execute_write(|conn| ScabbardStoreOperations::new(conn).remove_service(service_id))
    }

    /// Set a scabbard alarm
    fn set_alarm(
        &self,
        service_id: &FullyQualifiedServiceId,
        alarm_type: &AlarmType,
        alarm: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).set_alarm(service_id, alarm_type, alarm)
        })
    }

    /// Unset a scabbard alarm
    fn unset_alarm(
        &self,
        service_id: &FullyQualifiedServiceId,
        alarm_type: &AlarmType,
    ) -> Result<(), ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).unset_alarm(service_id, alarm_type)
        })
    }

    /// Get the scabbard alarm of a specified type for the given service
    fn get_alarm(
        &self,
        service_id: &FullyQualifiedServiceId,
        alarm_type: &AlarmType,
    ) -> Result<Option<SystemTime>, ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).get_alarm(service_id, alarm_type)
        })
    }
}

pub struct DieselConnectionScabbardStore<'a, C>
where
    C: diesel::Connection<TransactionManager = AnsiTransactionManager> + 'static,
    C::Backend: diesel::backend::UsesAnsiSavepointSyntax,
{
    connection: &'a C,
}

impl<'a, C> DieselConnectionScabbardStore<'a, C>
where
    C: diesel::Connection<TransactionManager = AnsiTransactionManager> + 'static,
    C::Backend: diesel::backend::UsesAnsiSavepointSyntax,
{
    pub fn new(connection: &'a C) -> Self {
        DieselConnectionScabbardStore { connection }
    }
}

#[cfg(feature = "sqlite")]
impl<'a> ScabbardStore for DieselConnectionScabbardStore<'a, SqliteConnection> {
    /// Add a new context
    fn add_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ConsensusContext,
    ) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).add_consensus_context(service_id, context)
    }
    /// Update an existing context
    fn update_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ConsensusContext,
    ) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).update_consensus_context(service_id, context)
    }
    /// Add a 2 phase commit coordinator action
    fn add_consensus_action(
        &self,
        action: ConsensusAction,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<i64, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).add_consensus_action(action, service_id)
    }
    /// Update an existing 2 phase commit action
    fn update_consensus_action(
        &self,
        service_id: &FullyQualifiedServiceId,
        action_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).update_consensus_action(
            service_id,
            action_id,
            executed_at,
        )
    }
    /// List all coordinator actions for a given service_id
    fn list_consensus_actions(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Vec<Identified<ConsensusAction>>, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).list_consensus_actions(service_id)
    }
    /// List ready services
    fn list_ready_services(&self) -> Result<Vec<FullyQualifiedServiceId>, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).list_ready_services()
    }
    /// Add a new scabbard service
    fn add_service(&self, service: ScabbardService) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).add_service(service)
    }
    /// Add a new commit entry
    fn add_commit_entry(&self, commit_entry: CommitEntry) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).add_commit_entry(commit_entry)
    }
    /// Get the commit entry for the specified service_id
    fn get_last_commit_entry(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<CommitEntry>, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).get_last_commit_entry(service_id)
    }
    /// Update an existing commit entry
    fn update_commit_entry(&self, commit_entry: CommitEntry) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).update_commit_entry(commit_entry)
    }
    /// Update an existing scabbard service
    fn update_service(&self, service: ScabbardService) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).update_service(service)
    }
    /// Get service
    fn get_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<ScabbardService>, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).get_service(service_id)
    }
    /// Add a new consensus event
    fn add_consensus_event(
        &self,
        service_id: &FullyQualifiedServiceId,
        event: ConsensusEvent,
    ) -> Result<i64, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).add_consensus_event(service_id, event)
    }
    /// Update an existing consensus event
    fn update_consensus_event(
        &self,
        service_id: &FullyQualifiedServiceId,
        event_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).update_consensus_event(
            service_id,
            event_id,
            executed_at,
        )
    }
    /// List all consensus events for a given service_id
    fn list_consensus_events(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Vec<Identified<ConsensusEvent>>, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).list_consensus_events(service_id)
    }
    /// Get the current context for a given service
    fn get_current_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<ConsensusContext>, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).get_current_consensus_context(service_id)
    }
    /// Remove existing service
    fn remove_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).remove_service(service_id)
    }

    /// Set a scabbard alarm
    fn set_alarm(
        &self,
        service_id: &FullyQualifiedServiceId,
        alarm_type: &AlarmType,
        alarm: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).set_alarm(service_id, alarm_type, alarm)
    }

    /// Unset a scabbard alarm
    fn unset_alarm(
        &self,
        service_id: &FullyQualifiedServiceId,
        alarm_type: &AlarmType,
    ) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).unset_alarm(service_id, alarm_type)
    }

    /// Get the scabbard alarm of a specified type for the given service
    fn get_alarm(
        &self,
        service_id: &FullyQualifiedServiceId,
        alarm_type: &AlarmType,
    ) -> Result<Option<SystemTime>, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).get_alarm(service_id, alarm_type)
    }
}

#[cfg(feature = "postgres")]
impl<'a> ScabbardStore for DieselConnectionScabbardStore<'a, PgConnection> {
    /// Add a new context
    fn add_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ConsensusContext,
    ) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).add_consensus_context(service_id, context)
    }
    /// Update an existing context
    fn update_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ConsensusContext,
    ) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).update_consensus_context(service_id, context)
    }
    /// Add a 2 phase commit coordinator action
    fn add_consensus_action(
        &self,
        action: ConsensusAction,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<i64, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).add_consensus_action(action, service_id)
    }
    /// Update an existing 2 phase commit action
    fn update_consensus_action(
        &self,
        service_id: &FullyQualifiedServiceId,
        action_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).update_consensus_action(
            service_id,
            action_id,
            executed_at,
        )
    }
    /// List all coordinator actions for a given service_id
    fn list_consensus_actions(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Vec<Identified<ConsensusAction>>, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).list_consensus_actions(service_id)
    }
    /// List ready services
    fn list_ready_services(&self) -> Result<Vec<FullyQualifiedServiceId>, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).list_ready_services()
    }
    /// Add a new scabbard service
    fn add_service(&self, service: ScabbardService) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).add_service(service)
    }
    /// Add a new commit entry
    fn add_commit_entry(&self, commit_entry: CommitEntry) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).add_commit_entry(commit_entry)
    }
    /// Get the commit entry for the specified service_id
    fn get_last_commit_entry(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<CommitEntry>, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).get_last_commit_entry(service_id)
    }
    /// Update an existing commit entry
    fn update_commit_entry(&self, commit_entry: CommitEntry) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).update_commit_entry(commit_entry)
    }
    /// Update an existing scabbard service
    fn update_service(&self, service: ScabbardService) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).update_service(service)
    }
    fn get_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<ScabbardService>, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).get_service(service_id)
    }
    /// Add a new consensus event
    fn add_consensus_event(
        &self,
        service_id: &FullyQualifiedServiceId,
        event: ConsensusEvent,
    ) -> Result<i64, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).add_consensus_event(service_id, event)
    }
    /// Update an existing consensus event
    fn update_consensus_event(
        &self,
        service_id: &FullyQualifiedServiceId,
        event_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).update_consensus_event(
            service_id,
            event_id,
            executed_at,
        )
    }
    /// List all consensus events for a given service_id
    fn list_consensus_events(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Vec<Identified<ConsensusEvent>>, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).list_consensus_events(service_id)
    }
    /// Get the current context for a given service
    fn get_current_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<ConsensusContext>, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).get_current_consensus_context(service_id)
    }
    /// Remove existing service
    fn remove_service(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).remove_service(service_id)
    }

    /// Set a scabbard alarm
    fn set_alarm(
        &self,
        service_id: &FullyQualifiedServiceId,
        alarm_type: &AlarmType,
        alarm: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).set_alarm(service_id, alarm_type, alarm)
    }

    /// Unset a scabbard alarm
    fn unset_alarm(
        &self,
        service_id: &FullyQualifiedServiceId,
        alarm_type: &AlarmType,
    ) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).unset_alarm(service_id, alarm_type)
    }

    /// Get the scabbard alarm of a specified type for the given service
    fn get_alarm(
        &self,
        service_id: &FullyQualifiedServiceId,
        alarm_type: &AlarmType,
    ) -> Result<Option<SystemTime>, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).get_alarm(service_id, alarm_type)
    }
}

#[cfg(all(test, feature = "sqlite"))]
pub mod tests {
    use super::*;

    use std::time::Duration;
    use std::time::SystemTime;

    use crate::migrations::run_sqlite_migrations;

    use crate::store::scabbard_store::{
        service::{ConsensusType, ScabbardServiceBuilder, ServiceStatus},
        two_phase_commit::{
            Action, ContextBuilder, Event, Message, Notification, Participant, State,
        },
        CommitEntryBuilder, ConsensusDecision,
    };

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };
    use splinter::{service::FullyQualifiedServiceId, service::ServiceId};

    /// Test that the scabbard store `add_consensus_context` operation is successful.
    ///
    /// 1. Add a valid context to the store and check that the operation is successful
    /// 2. Attempt to add the same context to the store again and check that an error is returned
    #[test]
    fn scabbard_store_add_context() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselScabbardStore::new(pool);

        let coordinator_fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let participant_service_id = ServiceId::new_random();

        let coordinator_context = ContextBuilder::default()
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.clone(),
                vote: None,
            }])
            .with_state(State::WaitingForStart)
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");
        let context = ConsensusContext::TwoPhaseCommit(coordinator_context);

        assert!(store
            .add_consensus_context(&coordinator_fqsi, context)
            .is_ok());

        let coordinator_context = ContextBuilder::default()
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.clone(),
                vote: None,
            }])
            .with_state(State::WaitingForStart)
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");
        let context = ConsensusContext::TwoPhaseCommit(coordinator_context);

        assert!(store
            .add_consensus_context(&coordinator_fqsi, context)
            .is_err());
    }

    /// Test that the scabbard store `add_consensus_action` operation is successful.
    ///
    /// 1. Add a valid context to the store
    /// 2. Add an action with the same `service_id` and `epoch` to the store and check
    ///    that the operation was successful
    #[test]
    fn scabbard_store_add_actions() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselScabbardStore::new(pool);

        let coordinator_fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let participant_service_id = ServiceId::new_random();

        let coordinator_context = ContextBuilder::default()
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.clone(),
                vote: None,
            }])
            .with_state(State::WaitingForStart)
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");
        let context = ConsensusContext::TwoPhaseCommit(coordinator_context);

        store
            .add_consensus_context(&coordinator_fqsi, context)
            .expect("failed to add context");

        let notification = Notification::RequestForStart();
        let action = ConsensusAction::TwoPhaseCommit(Action::Notify(notification));

        assert!(store
            .add_consensus_action(action, &coordinator_fqsi)
            .is_ok());
    }

    /// Test that the scabbard store `list_consensus_actions` operation is successful.
    ///
    /// 1. Add a valid context to the store
    /// 2. Add two actions with the same `service_id` and `epoch` to the store
    /// 3. List the actions and check that the actions returned match what was added
    #[test]
    fn scabbard_store_list_actions() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselScabbardStore::new(pool);

        let coordinator_fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let participant_service_id = ServiceId::new_random();

        let coordinator_context = ContextBuilder::default()
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.clone(),
                vote: None,
            }])
            .with_state(State::WaitingForStart)
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");
        let context = ConsensusContext::TwoPhaseCommit(coordinator_context);

        store
            .add_consensus_context(&coordinator_fqsi, context)
            .expect("failed to add context");

        let notification = Notification::RequestForStart();
        let action1 = ConsensusAction::TwoPhaseCommit(Action::Notify(notification));

        let vote_timeout_start = SystemTime::UNIX_EPOCH
            .checked_add(Duration::from_secs(
                SystemTime::now()
                    .checked_add(Duration::from_secs(120))
                    .expect("failed to get alarm time")
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .expect("failed to get duration since UNIX EPOCH")
                    .as_secs(),
            ))
            .unwrap();

        let update_context = ContextBuilder::default()
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.clone(),
                vote: None,
            }])
            .with_state(State::Voting { vote_timeout_start })
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build update context");

        let action2 = ConsensusAction::TwoPhaseCommit(Action::Update(
            ConsensusContext::TwoPhaseCommit(update_context),
            None,
        ));

        let action_id1 = store
            .add_consensus_action(action1, &coordinator_fqsi)
            .expect("failed to add actions");
        let action_id2 = store
            .add_consensus_action(action2, &coordinator_fqsi)
            .expect("failed to add actions");

        let action_list = store
            .list_consensus_actions(&coordinator_fqsi)
            .expect("failed to list all actions");

        let expected_update_context = ContextBuilder::default()
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.clone(),
                vote: None,
            }])
            .with_state(State::Voting { vote_timeout_start })
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build update context");

        assert_eq!(
            action_list[0],
            Identified {
                id: action_id1,
                record: ConsensusAction::TwoPhaseCommit(Action::Notify(
                    Notification::RequestForStart()
                )),
            },
        );
        assert_eq!(
            action_list[1],
            Identified {
                id: action_id2,
                record: ConsensusAction::TwoPhaseCommit(Action::Update(
                    ConsensusContext::TwoPhaseCommit(expected_update_context),
                    None,
                )),
            },
        );
    }

    /// Test that the scabbard store `update_consensus_context` operation is successful.
    ///
    /// 1. Add a valid context to the store
    /// 2. Create a valid updated context
    /// 3. Attempt to update the original context, check that the operation is successful
    #[test]
    fn scabbard_store_update_context() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselScabbardStore::new(pool);

        let coordinator_fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let participant_service_id = ServiceId::new_random();

        let coordinator_context = ContextBuilder::default()
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.clone(),
                vote: None,
            }])
            .with_state(State::WaitingForStart)
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");
        let context = ConsensusContext::TwoPhaseCommit(coordinator_context);

        store
            .add_consensus_context(&coordinator_fqsi, context)
            .expect("failed to add context");

        let vote_timeout_start = SystemTime::UNIX_EPOCH
            .checked_add(Duration::from_secs(
                SystemTime::now()
                    .checked_add(Duration::from_secs(120))
                    .expect("failed to get alarm time")
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .expect("failed to get duration since UNIX EPOCH")
                    .as_secs(),
            ))
            .unwrap();

        let update_context = ContextBuilder::default()
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.clone(),
                vote: None,
            }])
            .with_state(State::Voting { vote_timeout_start })
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build update context");

        assert!(store
            .update_consensus_context(
                &coordinator_fqsi,
                ConsensusContext::TwoPhaseCommit(update_context)
            )
            .is_ok());
    }

    /// Test that the scabbard store `update_consensus_action` operation is successful.
    ///
    /// 1. Add a valid context to the store
    /// 2. Add an action to the database with the same `service_id` and `epoch`
    /// 3. Attempt to update the action as if it had been executed, check that the operation is
    ///    successful
    #[test]
    fn scabbard_store_update_action() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselScabbardStore::new(pool);

        let coordinator_fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let participant_service_id = ServiceId::new_random();

        let coordinator_context = ContextBuilder::default()
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.clone(),
                vote: None,
            }])
            .with_state(State::WaitingForStart)
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");
        let context = ConsensusContext::TwoPhaseCommit(coordinator_context);

        store
            .add_consensus_context(&coordinator_fqsi, context)
            .expect("failed to add context");

        let vote_timeout_start = SystemTime::UNIX_EPOCH
            .checked_add(Duration::from_secs(
                SystemTime::now()
                    .checked_add(Duration::from_secs(120))
                    .expect("failed to get alarm time")
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .expect("failed to get duration since UNIX EPOCH")
                    .as_secs(),
            ))
            .unwrap();

        let update_context = ContextBuilder::default()
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.clone(),
                vote: None,
            }])
            .with_state(State::Voting { vote_timeout_start })
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build update context");
        let action = ConsensusAction::TwoPhaseCommit(Action::Update(
            ConsensusContext::TwoPhaseCommit(update_context),
            None,
        ));

        let action_id = store
            .add_consensus_action(action, &coordinator_fqsi)
            .expect("failed to add actions");

        assert!(store
            .update_consensus_action(&coordinator_fqsi, action_id, SystemTime::now())
            .is_ok());
    }

    /// Test that the scabbard store `get_service` operation is successful.
    ///
    /// 1. Add a service in the finalized state to the database
    /// 2. Fetch that service from the store
    /// 3. Verify the correct service was returned
    #[test]
    fn scabbard_store_get_service() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselScabbardStore::new(pool);

        let service_fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let peer_service_id = ServiceId::new_random();
        let peer_service_id_second = ServiceId::new_random();

        let mut peers = vec![peer_service_id, peer_service_id_second];
        peers.sort_by(|a, b| a.as_str().partial_cmp(b.as_str()).unwrap());

        // service with finalized status
        let service = ScabbardServiceBuilder::default()
            .with_service_id(&service_fqsi)
            .with_peers(&peers)
            .with_consensus(&ConsensusType::TwoPC)
            .with_status(&ServiceStatus::Finalized)
            .build()
            .expect("failed to build service");

        assert!(store.add_service(service.clone()).is_ok());

        let fetched_service = store
            .get_service(&service_fqsi)
            .expect("Unable to fetch service")
            .expect("Store should have returned a service");

        assert_eq!(service, fetched_service);
    }

    /// Test that the scabbard store `list_ready_services` operation is successful.
    ///
    /// 1. Add a service in the finalized state to the database
    /// 2. Add a context to the database for the service that has a past due alarm
    /// 3. Call `list_ready_services` and check that the service is returned
    /// 4. Update the context to set the alarm for one week in the future
    /// 5. Add an unexecuted action
    /// 6. Call `list_ready_services` and check that the service is returned
    /// 7. Update the action as if it had been executed
    /// 8. Call `list_ready_services` and check that no services are returned
    #[test]
    fn scabbard_store_list_ready_services() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselScabbardStore::new(pool);

        let service_fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let peer_service_id = ServiceId::new_random();

        // service with finalized status
        let service = ScabbardServiceBuilder::default()
            .with_service_id(&service_fqsi)
            .with_peers(&[peer_service_id.clone()])
            .with_consensus(&ConsensusType::TwoPC)
            .with_status(&ServiceStatus::Finalized)
            .build()
            .expect("failed to build service");

        assert!(store.add_service(service).is_ok());

        let coordinator_context = ContextBuilder::default()
            .with_coordinator(service_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: peer_service_id.clone(),
                vote: None,
            }])
            .with_state(State::WaitingForStart)
            .with_this_process(service_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");

        let context = ConsensusContext::TwoPhaseCommit(coordinator_context);

        store
            .add_consensus_context(&service_fqsi, context)
            .expect("failed to add context to store");

        store
            .set_alarm(&service_fqsi, &AlarmType::TwoPhaseCommit, SystemTime::now())
            .expect("failed to add alarm to store");

        let ready_services = store
            .list_ready_services()
            .expect("failed to list ready services");

        // check that the one service is returned because its alarm has passed
        assert_eq!(&ready_services[0], &service_fqsi);

        let notification = Notification::RequestForStart();
        let action = ConsensusAction::TwoPhaseCommit(Action::Notify(notification));

        let updated_alarm = SystemTime::now()
            .checked_add(Duration::from_secs(604800))
            .expect("failed to get alarm time");

        // update the context to have an alarm far in the future
        store
            .set_alarm(&service_fqsi, &AlarmType::TwoPhaseCommit, updated_alarm)
            .expect("failed to add alarm to store");

        // add an action for the service
        let action_id = store
            .add_consensus_action(action, &service_fqsi)
            .expect("failed to add actions");

        let ready_services = store
            .list_ready_services()
            .expect("failed to list ready services");

        // check that the one service is still returned because it has an unexecuted action
        assert_eq!(&ready_services[0], &service_fqsi);

        store
            .update_consensus_action(&service_fqsi, action_id, SystemTime::now())
            .expect("failed to update action");

        // check that no services are returned because there are no un-exectuted actions or
        // ready alarms
        let ready_services = store
            .list_ready_services()
            .expect("failed to list ready services");

        assert!(ready_services.is_empty());
    }

    /// Test that the scabbard store `add_commit_entry` operation is successful.
    ///
    /// 1. Add a valid service to the database
    /// 2. Add a commit entry with the same `service_id` and `epoch` as the service to the
    ///    database and check that the operation was successful
    /// 3. Use `get_last_commit_entry` and check that the returned commit entry matches the one
    ///    that was added
    /// 4. Attempt to add a commit entry for a service that does not exist and check that an error
    ///    is returned
    #[test]
    fn scabbard_add_commit_entry() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselScabbardStore::new(pool);

        let service_fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let peer_service_id = ServiceId::new_random();

        // service with finalized status
        let service = ScabbardServiceBuilder::default()
            .with_service_id(&service_fqsi)
            .with_peers(&[peer_service_id.clone()])
            .with_consensus(&ConsensusType::TwoPC)
            .with_status(&ServiceStatus::Finalized)
            .build()
            .expect("failed to build service");

        store.add_service(service).expect("faield to add service");

        let coordinator_context = ContextBuilder::default()
            .with_coordinator(service_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: peer_service_id.clone(),
                vote: None,
            }])
            .with_state(State::WaitingForStart)
            .with_this_process(service_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");

        let context = ConsensusContext::TwoPhaseCommit(coordinator_context);

        store
            .add_consensus_context(&service_fqsi, context)
            .expect("failed to add context to store");

        let commit_entry = CommitEntryBuilder::default()
            .with_service_id(&service_fqsi)
            .with_epoch(1)
            .with_value("commit entry value")
            .with_decision(&ConsensusDecision::Commit)
            .build()
            .expect("failed to build commit entry");

        assert!(store.add_commit_entry(commit_entry.clone()).is_ok());

        let get_last_commit_entry = store
            .get_last_commit_entry(&service_fqsi.clone())
            .expect("failed to get commit entry");

        assert_eq!(get_last_commit_entry, Some(commit_entry));

        let bad_commit_entry = CommitEntryBuilder::default()
            .with_service_id(&FullyQualifiedServiceId::new_random())
            .with_epoch(1)
            .with_value("commit entry value")
            .with_decision(&ConsensusDecision::Commit)
            .build()
            .expect("failed to build commit entry");

        assert!(store.add_commit_entry(bad_commit_entry).is_err());
    }

    /// Test that the scabbard store `get_last_commit_entry` operation is successful.
    ///
    /// 1. Add a valid service to the database
    /// 2. Add a commit entry with the same `service_id` and `epoch` as the service to the
    ///    database and check that the operation was successful
    /// 3. Use `get_last_commit_entry` and check that the returned commit entry matches the one
    ///    that was added
    /// 4. Attempt to call `get_last_commit_entry` with a `service_id` for a service that does not
    ///    exist and check that `None` is returned
    #[test]
    fn scabbard_get_last_commit_entry() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselScabbardStore::new(pool);

        let service_fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let peer_service_id = ServiceId::new_random();

        // service with finalized status
        let service = ScabbardServiceBuilder::default()
            .with_service_id(&service_fqsi)
            .with_peers(&[peer_service_id.clone()])
            .with_consensus(&ConsensusType::TwoPC)
            .with_status(&ServiceStatus::Finalized)
            .build()
            .expect("failed to build service");

        store.add_service(service).expect("faield to add service");

        let coordinator_context = ContextBuilder::default()
            .with_coordinator(service_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: peer_service_id.clone(),
                vote: None,
            }])
            .with_state(State::WaitingForStart)
            .with_this_process(service_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");

        let context = ConsensusContext::TwoPhaseCommit(coordinator_context);

        store
            .add_consensus_context(&service_fqsi, context)
            .expect("failed to add context to store");

        let commit_entry = CommitEntryBuilder::default()
            .with_service_id(&service_fqsi)
            .with_epoch(1)
            .with_value("commit entry value")
            .with_decision(&ConsensusDecision::Commit)
            .build()
            .expect("failed to build commit entry");

        assert!(store.add_commit_entry(commit_entry.clone()).is_ok());

        let get_last_commit_entry = store
            .get_last_commit_entry(&service_fqsi.clone())
            .expect("failed to get commit entry");

        assert_eq!(get_last_commit_entry, Some(commit_entry));

        let get_last_commit_entry = store
            .get_last_commit_entry(&FullyQualifiedServiceId::new_random())
            .expect("failed to get commit entry");

        assert_eq!(get_last_commit_entry, None);
    }

    /// Test that the scabbard store `update_commit_entry` operation is successful.
    ///
    /// 1. Add a valid service to the database
    /// 2. Add a commit entry with the same `service_id` and `epoch` as the service to the
    ///    database and check that the operation was successful
    /// 3. Attempt to update the commit entry in the database and check that the operation is
    ///    successful
    /// 4. Use `get_last_commit_entry` and check that the returned commit entry matches the
    ///    updated commit entry
    #[test]
    fn scabbard_update_commit_entry() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselScabbardStore::new(pool);

        let service_fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let peer_service_id = ServiceId::new_random();

        // service with finalized status
        let service = ScabbardServiceBuilder::default()
            .with_service_id(&service_fqsi)
            .with_peers(&[peer_service_id.clone()])
            .with_consensus(&ConsensusType::TwoPC)
            .with_status(&ServiceStatus::Finalized)
            .build()
            .expect("failed to build service");

        store.add_service(service).expect("faield to add service");

        let coordinator_context = ContextBuilder::default()
            .with_coordinator(service_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: peer_service_id.clone(),
                vote: None,
            }])
            .with_state(State::WaitingForStart)
            .with_this_process(service_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");

        let context = ConsensusContext::TwoPhaseCommit(coordinator_context);

        store
            .add_consensus_context(&service_fqsi, context)
            .expect("failed to add context to store");

        let commit_entry = CommitEntryBuilder::default()
            .with_service_id(&service_fqsi)
            .with_epoch(1)
            .with_value("commit entry value")
            .build()
            .expect("failed to build commit entry");

        store
            .add_commit_entry(commit_entry.clone())
            .expect("failed to add entry");

        let update_commit_entry = CommitEntryBuilder::default()
            .with_service_id(&service_fqsi)
            .with_epoch(1)
            .with_value("commit entry value")
            .with_decision(&ConsensusDecision::Commit)
            .build()
            .expect("failed to build commit entry");

        assert!(store
            .update_commit_entry(update_commit_entry.clone())
            .is_ok());

        let get_last_commit_entry = store
            .get_last_commit_entry(&service_fqsi.clone())
            .expect("failed to get commit entry");

        assert_eq!(get_last_commit_entry, Some(update_commit_entry));
    }

    /// Test that the scabbard store `update_service` operation is successful.
    ///
    /// 1. Add a service in the prepared state to the database
    /// 2. Add a context to the database for the service that has a past due alarm
    /// 3. Call `list_ready_services` and check that the service is not returned
    /// 4. Attempt to use `update_service` the service to change the service state to finalized
    /// 5. Call `list_ready_services` and check that the service is returned
    #[test]
    fn scabbard_store_update_service() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselScabbardStore::new(pool);

        let service_fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let peer_service_id = ServiceId::new_random();

        // service with finalized status
        let service = ScabbardServiceBuilder::default()
            .with_service_id(&service_fqsi)
            .with_peers(&[peer_service_id.clone()])
            .with_consensus(&ConsensusType::TwoPC)
            .with_status(&ServiceStatus::Prepared)
            .build()
            .expect("failed to build service");

        assert!(store.add_service(service.clone()).is_ok());

        let coordinator_context = ContextBuilder::default()
            .with_coordinator(service_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: peer_service_id.clone(),
                vote: None,
            }])
            .with_state(State::WaitingForStart)
            .with_this_process(service_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");

        let context = ConsensusContext::TwoPhaseCommit(coordinator_context);

        store
            .add_consensus_context(&service_fqsi, context)
            .expect("failed to add context to store");

        store
            .set_alarm(&service_fqsi, &AlarmType::TwoPhaseCommit, SystemTime::now())
            .expect("failed to add alarm to store");

        let ready_services = store
            .list_ready_services()
            .expect("failed to list ready services");

        // Check that the service is not returned yet because it is in the prepared state still
        assert!(ready_services.is_empty());

        let update_service = service
            .into_builder()
            .with_status(&ServiceStatus::Finalized)
            .build()
            .expect("failed to build update service");

        assert!(store.update_service(update_service).is_ok());

        let ready_services = store
            .list_ready_services()
            .expect("failed to list ready services");

        // check that the service is returned because it is in the finalized state now
        assert_eq!(&ready_services[0], &service_fqsi);
    }

    /// Test that the scabbard store `add_consensus_event` operation is successful.
    ///
    /// 1. Add a valid participant context to the store
    /// 2. Attempt to add a valid event to the store and check that the operation was successful
    /// 3. Attempt to add a an event for a service_id that does not exist check that an error is
    ///    returned
    #[test]
    fn scabbard_store_add_event() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselScabbardStore::new(pool);

        let coordinator_fqsi = FullyQualifiedServiceId::new_random();

        let participant_fqsi = FullyQualifiedServiceId::new_random();
        let participant2_fqsi = FullyQualifiedServiceId::new_random();

        let participant_context = ContextBuilder::default()
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![
                Participant {
                    process: participant_fqsi.service_id().clone(),
                    vote: None,
                },
                Participant {
                    process: participant2_fqsi.service_id().clone(),
                    vote: None,
                },
            ])
            .with_state(State::WaitingForVoteRequest)
            .with_this_process(participant_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");
        let context = ConsensusContext::TwoPhaseCommit(participant_context);

        store
            .add_consensus_context(&participant_fqsi, context)
            .expect("failed to add context");

        let event = ConsensusEvent::TwoPhaseCommit(Event::Deliver(
            participant2_fqsi.service_id().clone(),
            Message::DecisionRequest(1),
        ));

        assert!(store
            .add_consensus_event(&participant_fqsi, event.clone())
            .is_ok());

        assert!(store
            .add_consensus_event(&participant2_fqsi, event)
            .is_err());
    }

    /// Test that the scabbard store `update_consensus_event` operation is successful.
    ///
    /// 1. Add a valid participant context to the store
    /// 2. Add a valid event to the store
    /// 3. Attempt to update the `executed_at` time for the event and check that the operation was
    ///    successful
    #[test]
    fn scabbard_store_update_event() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselScabbardStore::new(pool);

        let coordinator_fqsi = FullyQualifiedServiceId::new_random();

        let participant_fqsi = FullyQualifiedServiceId::new_random();
        let participant2_fqsi = FullyQualifiedServiceId::new_random();

        let participant_context = ContextBuilder::default()
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![
                Participant {
                    process: participant_fqsi.service_id().clone(),
                    vote: None,
                },
                Participant {
                    process: participant2_fqsi.service_id().clone(),
                    vote: None,
                },
            ])
            .with_state(State::WaitingForVoteRequest)
            .with_this_process(participant_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");
        let context = ConsensusContext::TwoPhaseCommit(participant_context);

        store
            .add_consensus_context(&participant_fqsi, context)
            .expect("failed to add context");

        let event = ConsensusEvent::TwoPhaseCommit(Event::Deliver(
            participant2_fqsi.service_id().clone(),
            Message::DecisionRequest(1),
        ));

        let event_id = store
            .add_consensus_event(&participant_fqsi, event)
            .expect("failed to add event");

        assert!(store
            .update_consensus_event(&participant_fqsi, event_id, SystemTime::now())
            .is_ok());
    }

    /// Test that the scabbard store `list_consensus_events` operation is successful.
    ///
    /// 1. Add a valid participant context to the store
    /// 2. Add a valid event to the store
    /// 3. Call `list_consensus_events` and check that the one event is returned
    /// 4. Add a second valid event to the store
    /// 5. Call `list_consensus_events` and check that both events are returned in the correct order
    /// 6. Update the `executed_at` time for the first event
    /// 7. Call `list_consensus_events` and check that only the second event is returned
    #[test]
    fn scabbard_store_list_events() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselScabbardStore::new(pool);

        let coordinator_fqsi = FullyQualifiedServiceId::new_random();

        let participant_fqsi = FullyQualifiedServiceId::new_random();
        let participant2_fqsi = FullyQualifiedServiceId::new_random();

        let participant_context = ContextBuilder::default()
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![
                Participant {
                    process: participant_fqsi.service_id().clone(),
                    vote: None,
                },
                Participant {
                    process: participant2_fqsi.service_id().clone(),
                    vote: None,
                },
            ])
            .with_state(State::WaitingForVoteRequest)
            .with_this_process(participant_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");
        let context = ConsensusContext::TwoPhaseCommit(participant_context);

        store
            .add_consensus_context(&participant_fqsi, context)
            .expect("failed to add context");

        let event = ConsensusEvent::TwoPhaseCommit(Event::Deliver(
            participant2_fqsi.service_id().clone(),
            Message::DecisionRequest(1),
        ));

        let event_id = store
            .add_consensus_event(&participant_fqsi, event)
            .expect("failed to add event");

        let events = store
            .list_consensus_events(&participant_fqsi)
            .expect("failed to list events");

        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            Identified {
                id: event_id,
                record: ConsensusEvent::TwoPhaseCommit(Event::Deliver(
                    participant2_fqsi.service_id().clone(),
                    Message::DecisionRequest(1)
                )),
            },
        );

        let event2 = ConsensusEvent::TwoPhaseCommit(Event::Alarm());

        let event_id2 = store
            .add_consensus_event(&participant_fqsi, event2)
            .expect("failed to add event");

        let events = store
            .list_consensus_events(&participant_fqsi)
            .expect("failed to list events");

        assert_eq!(events.len(), 2);
        assert_eq!(
            events[0],
            Identified {
                id: event_id,
                record: ConsensusEvent::TwoPhaseCommit(Event::Deliver(
                    participant2_fqsi.service_id().clone(),
                    Message::DecisionRequest(1)
                )),
            },
        );
        assert_eq!(
            events[1],
            Identified {
                id: event_id2,
                record: ConsensusEvent::TwoPhaseCommit(Event::Alarm()),
            },
        );

        store
            .update_consensus_event(&participant_fqsi, event_id, SystemTime::now())
            .expect("failed to update event");

        let events = store
            .list_consensus_events(&participant_fqsi)
            .expect("failed to list events");

        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            Identified {
                id: event_id2,
                record: ConsensusEvent::TwoPhaseCommit(Event::Alarm()),
            },
        );
    }

    /// Test that the scabbard store `get_current_consensus_context` operation is successful.
    ///
    /// 1. Add two services to the database
    /// 2. Add a coordinator context for the first service
    /// 3. Add a participant context for the second service
    /// 4. Call `get_current_consensus_context` with the first service ID and check that the
    ///    coordinator context is returned
    /// 5. Call `get_current_consensus_context` with the second service ID and check that the
    ///    participant context is returned
    /// 6. Add a second coordinator context for the first service with a larger epoch
    /// 7. Call `get_current_consensus_context` with the first service ID and check that the
    ///    coordinator context with the larger epoch is returned
    /// 8. Add a participant context with a larger epoch for the first service
    /// 9. Call `get_current_consensus_context` with the first service ID and check that the
    ///    participant context that was most recently added is returned
    #[test]
    fn scabbard_store_get_current_context() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselScabbardStore::new(pool);

        let coordinator_fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let participant_service_id = FullyQualifiedServiceId::new_random();

        let service = ScabbardServiceBuilder::default()
            .with_service_id(&coordinator_fqsi)
            .with_peers(&[participant_service_id.service_id().clone()])
            .with_status(&ServiceStatus::Finalized)
            .with_consensus(&ConsensusType::TwoPC)
            .build()
            .expect("failed to build service");

        store.add_service(service).expect("faield to add service");

        let service = ScabbardServiceBuilder::default()
            .with_service_id(&participant_service_id)
            .with_peers(&[coordinator_fqsi.service_id().clone()])
            .with_status(&ServiceStatus::Finalized)
            .with_consensus(&ConsensusType::TwoPC)
            .build()
            .expect("failed to build service");

        store.add_service(service).expect("faield to add service");

        let coordinator_context = ContextBuilder::default()
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.service_id().clone(),
                vote: None,
            }])
            .with_state(State::WaitingForStart)
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");
        let context = ConsensusContext::TwoPhaseCommit(coordinator_context);

        store
            .add_consensus_context(&coordinator_fqsi, context.clone())
            .expect("failed to add context");

        let participant_context = ContextBuilder::default()
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.service_id().clone(),
                vote: None,
            }])
            .with_state(State::WaitingForVoteRequest)
            .with_this_process(participant_service_id.clone().service_id())
            .build()
            .expect("failed to build context");
        let context2 = ConsensusContext::TwoPhaseCommit(participant_context);

        store
            .add_consensus_context(&participant_service_id.clone(), context2.clone())
            .expect("failed to add context");

        let current_context = store
            .get_current_consensus_context(&coordinator_fqsi)
            .expect("failed to get current context");

        assert_eq!(current_context, Some(context));

        let current_context = store
            .get_current_consensus_context(&participant_service_id)
            .expect("failed to get current context");

        assert_eq!(current_context, Some(context2));

        let coordinator_context = ContextBuilder::default()
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(2)
            .with_participants(vec![Participant {
                process: participant_service_id.service_id().clone(),
                vote: None,
            }])
            .with_state(State::WaitingForStart)
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");
        let context3 = ConsensusContext::TwoPhaseCommit(coordinator_context);

        store
            .update_consensus_context(&coordinator_fqsi, context3.clone())
            .expect("failed to add context");

        let current_context = store
            .get_current_consensus_context(&coordinator_fqsi)
            .expect("failed to get current context");

        assert_eq!(current_context, Some(context3));

        let participant_context = ContextBuilder::default()
            .with_coordinator(participant_service_id.clone().service_id())
            .with_epoch(3)
            .with_participants(vec![Participant {
                process: coordinator_fqsi.service_id().clone(),
                vote: None,
            }])
            .with_state(State::WaitingForVoteRequest)
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");
        let context4 = ConsensusContext::TwoPhaseCommit(participant_context);

        store
            .update_consensus_context(&coordinator_fqsi.clone(), context4.clone())
            .expect("failed to add context");

        let current_context = store
            .get_current_consensus_context(&coordinator_fqsi)
            .expect("failed to get current context");

        assert_eq!(current_context, Some(context4));
    }

    /// Test that the scabbard store `remove_service` operation is successful.
    ///
    /// 1. Add a services to the database
    /// 2. Add a coordinator context for the service
    /// 3. Verify both can be fetched
    /// 4. Remove the service
    /// 5. Verify that the service and context were both removed
    #[test]
    fn scabbard_store_remove_service() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselScabbardStore::new(pool);

        let coordinator_fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let participant_service_id = FullyQualifiedServiceId::new_random();

        let service = ScabbardServiceBuilder::default()
            .with_service_id(&coordinator_fqsi)
            .with_peers(&[participant_service_id.service_id().clone()])
            .with_status(&ServiceStatus::Finalized)
            .with_consensus(&ConsensusType::TwoPC)
            .build()
            .expect("failed to build service");

        store
            .add_service(service.clone())
            .expect("failed to add service");

        let coordinator_context = ContextBuilder::default()
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.service_id().clone(),
                vote: None,
            }])
            .with_state(State::WaitingForStart)
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");
        let context = ConsensusContext::TwoPhaseCommit(coordinator_context);

        store
            .add_consensus_context(&coordinator_fqsi, context.clone())
            .expect("failed to add context");

        store
            .set_alarm(
                &coordinator_fqsi,
                &AlarmType::TwoPhaseCommit,
                SystemTime::now(),
            )
            .expect("failed to add alarm to store");

        let fetched_service = store
            .get_service(&coordinator_fqsi)
            .expect("failed to get service");

        assert_eq!(fetched_service, Some(service));

        let current_context = store
            .get_current_consensus_context(&coordinator_fqsi)
            .expect("failed to get current context");

        assert_eq!(current_context, Some(context));

        store
            .remove_service(&coordinator_fqsi)
            .expect("failed to get current context");

        assert!(store
            .get_service(&coordinator_fqsi)
            .expect("failed to get current context")
            .is_none());

        assert!(store
            .get_current_consensus_context(&coordinator_fqsi)
            .expect("failed to get current context")
            .is_none());
    }

    /// Test that the scabbard store `set_alarm` operation is successful.
    ///
    /// 1. Add a service to the database
    /// 2. Add a context to the database for the service
    /// 3. Call `list_ready_services` and check that the service is not returned
    /// 4. Set an alarm for the service and check that the operation was successful
    /// 5. Call `list_ready_services` and check that the service is returned because it
    ///    has an alarm that has passed
    #[test]
    fn scabbard_store_set_alarm() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselScabbardStore::new(pool);

        let service_fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let peer_service_id = ServiceId::new_random();

        // service with finalized status
        let service = ScabbardServiceBuilder::default()
            .with_service_id(&service_fqsi)
            .with_peers(&[peer_service_id.clone()])
            .with_consensus(&ConsensusType::TwoPC)
            .with_status(&ServiceStatus::Finalized)
            .build()
            .expect("failed to build service");

        assert!(store.add_service(service.clone()).is_ok());

        let coordinator_context = ContextBuilder::default()
            .with_coordinator(service_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: peer_service_id.clone(),
                vote: None,
            }])
            .with_state(State::WaitingForStart)
            .with_this_process(service_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");

        let context = ConsensusContext::TwoPhaseCommit(coordinator_context);

        store
            .add_consensus_context(&service_fqsi, context)
            .expect("failed to add context to store");

        let ready_services = store
            .list_ready_services()
            .expect("failed to list ready services");

        // Check that the service is not returned yet because it does not have an alarm that has
        // passed or outstanding actions
        assert!(ready_services.is_empty());

        assert!(store
            .set_alarm(&service_fqsi, &AlarmType::TwoPhaseCommit, SystemTime::now())
            .is_ok());

        let ready_services = store
            .list_ready_services()
            .expect("failed to list ready services");

        // check that the service is returned because it has an alarm that has passed
        assert_eq!(&ready_services[0], &service_fqsi);
    }

    /// Test that the scabbard store `unset_alarm` operation is successful.
    ///
    /// 1. Add a service to the database
    /// 2. Add a context to the database for the service
    /// 3. Call `list_ready_services` and check that the service is not returned
    /// 4. Set an alarm for the service and check that the operation was successful
    /// 5. Call `list_ready_services` and check that the service is returned because it
    ///    has an alarm that has passed
    /// 6. Unset the alarm for the service
    /// 7. Call `list_ready_services` and check that the service is no longer returned
    #[test]
    fn scabbard_store_unset_alarm() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselScabbardStore::new(pool);

        let service_fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let peer_service_id = ServiceId::new_random();

        // service with finalized status
        let service = ScabbardServiceBuilder::default()
            .with_service_id(&service_fqsi)
            .with_peers(&[peer_service_id.clone()])
            .with_consensus(&ConsensusType::TwoPC)
            .with_status(&ServiceStatus::Finalized)
            .build()
            .expect("failed to build service");

        assert!(store.add_service(service.clone()).is_ok());

        let coordinator_context = ContextBuilder::default()
            .with_coordinator(service_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: peer_service_id.clone(),
                vote: None,
            }])
            .with_state(State::WaitingForStart)
            .with_this_process(service_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");

        let context = ConsensusContext::TwoPhaseCommit(coordinator_context);

        store
            .add_consensus_context(&service_fqsi, context)
            .expect("failed to add context to store");

        let ready_services = store
            .list_ready_services()
            .expect("failed to list ready services");

        // Check that the service is not returned yet because it does not have a passed
        // alarm or outstanding actions
        assert!(ready_services.is_empty());

        store
            .set_alarm(&service_fqsi, &AlarmType::TwoPhaseCommit, SystemTime::now())
            .expect("failed to set alarm");

        let ready_services = store
            .list_ready_services()
            .expect("failed to list ready services");

        // check that the service is returned because it has an alarm that has passed
        assert_eq!(&ready_services[0], &service_fqsi);

        // unset the alarm that was just set
        assert!(store
            .unset_alarm(&service_fqsi, &AlarmType::TwoPhaseCommit)
            .is_ok());

        let ready_services = store
            .list_ready_services()
            .expect("failed to list ready services");

        // Check that the service is not returned because it does not have an alarm that has passed
        // or outstanding actions
        assert!(ready_services.is_empty());
    }

    /// Test that the scabbard store `get_alarm` operation is successful.
    ///
    /// 1. Add a service to the database
    /// 2. Add a context to the database for the service
    /// 3. Set an alarm for the service
    /// 4. Call `get_alarm` and check that the alarm is returned
    /// 5. Unset the alarm
    /// 6. Call `get_alarm` and check that the alarm is no longer returned
    #[test]
    fn scabbard_store_get_alarm() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselScabbardStore::new(pool);

        let service_fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let peer_service_id = ServiceId::new_random();

        // service with finalized status
        let service = ScabbardServiceBuilder::default()
            .with_service_id(&service_fqsi)
            .with_peers(&[peer_service_id.clone()])
            .with_consensus(&ConsensusType::TwoPC)
            .with_status(&ServiceStatus::Finalized)
            .build()
            .expect("failed to build service");

        assert!(store.add_service(service.clone()).is_ok());

        let coordinator_context = ContextBuilder::default()
            .with_coordinator(service_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: peer_service_id.clone(),
                vote: None,
            }])
            .with_state(State::WaitingForStart)
            .with_this_process(service_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");

        let context = ConsensusContext::TwoPhaseCommit(coordinator_context);

        store
            .add_consensus_context(&service_fqsi, context)
            .expect("failed to add context to store");

        let alarm = SystemTime::UNIX_EPOCH
            .checked_add(Duration::from_secs(
                SystemTime::now()
                    .checked_add(Duration::from_secs(60))
                    .expect("failed to get alarm time")
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .expect("failed to get duration since UNIX EPOCH")
                    .as_secs(),
            ))
            .unwrap();

        store
            .set_alarm(&service_fqsi, &AlarmType::TwoPhaseCommit, alarm)
            .expect("failed to set alarm");

        let retrieved_alarm = store
            .get_alarm(&service_fqsi, &AlarmType::TwoPhaseCommit)
            .expect("failed to list ready services");

        // check that the alarm is returned
        assert_eq!(retrieved_alarm, Some(alarm));

        // unset the alarm
        store
            .unset_alarm(&service_fqsi, &AlarmType::TwoPhaseCommit)
            .expect("failed to unset alarm");

        let retrieved_alarm = store
            .get_alarm(&service_fqsi, &AlarmType::TwoPhaseCommit)
            .expect("failed to list ready services");

        // Check that the alarm is not returned because it was unset
        assert_eq!(retrieved_alarm, None);
    }

    fn create_connection_pool_and_migrate() -> Pool<ConnectionManager<SqliteConnection>> {
        let connection_manager = ConnectionManager::<SqliteConnection>::new(":memory:");
        let pool = Pool::builder()
            .max_size(1)
            .build(connection_manager)
            .expect("Failed to build connection pool");

        run_sqlite_migrations(&*pool.get().expect("Failed to get connection for migrations"))
            .expect("Failed to run migrations");

        pool
    }
}
