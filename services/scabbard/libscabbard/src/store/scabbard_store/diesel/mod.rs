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
    commit::CommitEntry,
    event::{ReturnedScabbardConsensusEvent, ScabbardConsensusEvent},
    service::ScabbardService,
    ScabbardConsensusAction, ScabbardContext,
};

use super::ScabbardStore;

use operations::add_commit_entry::AddCommitEntryOperation as _;
use operations::add_consensus_action::AddActionOperation as _;
use operations::add_consensus_context::AddContextOperation as _;
use operations::add_consensus_event::AddEventOperation as _;
use operations::add_service::AddServiceOperation as _;
use operations::get_last_commit_entry::GetLastCommitEntryOperation as _;
use operations::get_service::GetServiceOperation as _;
use operations::list_consensus_actions::ListActionsOperation as _;
use operations::list_consensus_events::ListEventsOperation as _;
use operations::list_ready_services::ListReadyServicesOperation as _;
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
        context: ScabbardContext,
    ) -> Result<(), ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).add_consensus_context(service_id, context)
        })
    }
    /// Update an existing context
    fn update_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ScabbardContext,
    ) -> Result<(), ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).update_consensus_context(service_id, context)
        })
    }
    /// Add a 2 phase commit coordinator action
    fn add_consensus_action(
        &self,
        action: ScabbardConsensusAction,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<i64, ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).add_consensus_action(action, service_id, epoch)
        })
    }
    /// Update an existing 2 phase commit action
    fn update_consensus_action(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
        action_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).update_consensus_action(
                service_id,
                epoch,
                action_id,
                executed_at,
            )
        })
    }
    /// List all coordinator actions for a given service_id and epoch
    fn list_consensus_actions(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<Vec<ScabbardConsensusAction>, ScabbardStoreError> {
        self.pool.execute_read(|conn| {
            ScabbardStoreOperations::new(conn).list_consensus_actions(service_id, epoch)
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
    /// Get the commit entry for the specified service_id and epoch
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
        epoch: u64,
        event: ScabbardConsensusEvent,
    ) -> Result<i64, ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).add_consensus_event(service_id, epoch, event)
        })
    }
    /// Update an existing consensus event
    fn update_consensus_event(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
        event_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).update_consensus_event(
                service_id,
                epoch,
                event_id,
                executed_at,
            )
        })
    }
    /// List all consensus events for a given service_id and epoch
    fn list_consensus_events(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<Vec<ReturnedScabbardConsensusEvent>, ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).list_consensus_events(service_id, epoch)
        })
    }
}

#[cfg(feature = "postgres")]
impl ScabbardStore for DieselScabbardStore<PgConnection> {
    /// Add a new context
    fn add_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ScabbardContext,
    ) -> Result<(), ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).add_consensus_context(service_id, context)
        })
    }
    /// Update an existing context
    fn update_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ScabbardContext,
    ) -> Result<(), ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).update_consensus_context(service_id, context)
        })
    }
    /// Add a 2 phase commit coordinator action
    fn add_consensus_action(
        &self,
        action: ScabbardConsensusAction,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<i64, ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).add_consensus_action(action, service_id, epoch)
        })
    }
    /// Update an existing 2 phase commit action
    fn update_consensus_action(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
        action_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).update_consensus_action(
                service_id,
                epoch,
                action_id,
                executed_at,
            )
        })
    }
    /// List all coordinator actions for a given service_id and epoch
    fn list_consensus_actions(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<Vec<ScabbardConsensusAction>, ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).list_consensus_actions(service_id, epoch)
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
    /// Get the commit entry for the specified service_id and epoch
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
        epoch: u64,
        event: ScabbardConsensusEvent,
    ) -> Result<i64, ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).add_consensus_event(service_id, epoch, event)
        })
    }
    /// Update an existing consensus event
    fn update_consensus_event(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
        event_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).update_consensus_event(
                service_id,
                epoch,
                event_id,
                executed_at,
            )
        })
    }
    /// List all consensus events for a given service_id and epoch
    fn list_consensus_events(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<Vec<ReturnedScabbardConsensusEvent>, ScabbardStoreError> {
        self.pool.execute_write(|conn| {
            ScabbardStoreOperations::new(conn).list_consensus_events(service_id, epoch)
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
        context: ScabbardContext,
    ) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).add_consensus_context(service_id, context)
    }
    /// Update an existing context
    fn update_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ScabbardContext,
    ) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).update_consensus_context(service_id, context)
    }
    /// Add a 2 phase commit coordinator action
    fn add_consensus_action(
        &self,
        action: ScabbardConsensusAction,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<i64, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection)
            .add_consensus_action(action, service_id, epoch)
    }
    /// Update an existing 2 phase commit action
    fn update_consensus_action(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
        action_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).update_consensus_action(
            service_id,
            epoch,
            action_id,
            executed_at,
        )
    }
    /// List all coordinator actions for a given service_id and epoch
    fn list_consensus_actions(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<Vec<ScabbardConsensusAction>, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).list_consensus_actions(service_id, epoch)
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
    /// Get the commit entry for the specified service_id and epoch
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
        epoch: u64,
        event: ScabbardConsensusEvent,
    ) -> Result<i64, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).add_consensus_event(service_id, epoch, event)
    }
    /// Update an existing consensus event
    fn update_consensus_event(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
        event_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).update_consensus_event(
            service_id,
            epoch,
            event_id,
            executed_at,
        )
    }
    /// List all consensus events for a given service_id and epoch
    fn list_consensus_events(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<Vec<ReturnedScabbardConsensusEvent>, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).list_consensus_events(service_id, epoch)
    }
}

#[cfg(feature = "postgres")]
impl<'a> ScabbardStore for DieselConnectionScabbardStore<'a, PgConnection> {
    /// Add a new context
    fn add_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ScabbardContext,
    ) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).add_consensus_context(service_id, context)
    }
    /// Update an existing context
    fn update_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ScabbardContext,
    ) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).update_consensus_context(service_id, context)
    }
    /// Add a 2 phase commit coordinator action
    fn add_consensus_action(
        &self,
        action: ScabbardConsensusAction,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<i64, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection)
            .add_consensus_action(action, service_id, epoch)
    }
    /// Update an existing 2 phase commit action
    fn update_consensus_action(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
        action_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).update_consensus_action(
            service_id,
            epoch,
            action_id,
            executed_at,
        )
    }
    /// List all coordinator actions for a given service_id and epoch
    fn list_consensus_actions(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<Vec<ScabbardConsensusAction>, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).list_consensus_actions(service_id, epoch)
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
    /// Get the commit entry for the specified service_id and epoch
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
        epoch: u64,
        event: ScabbardConsensusEvent,
    ) -> Result<i64, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).add_consensus_event(service_id, epoch, event)
    }
    /// Update an existing consensus event
    fn update_consensus_event(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
        event_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).update_consensus_event(
            service_id,
            epoch,
            event_id,
            executed_at,
        )
    }
    /// List all consensus events for a given service_id and epoch
    fn list_consensus_events(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<Vec<ReturnedScabbardConsensusEvent>, ScabbardStoreError> {
        ScabbardStoreOperations::new(self.connection).list_consensus_events(service_id, epoch)
    }
}

#[cfg(all(test, feature = "sqlite"))]
pub mod tests {
    use super::*;

    use std::time::Duration;
    use std::time::SystemTime;

    use crate::migrations::run_sqlite_migrations;

    use crate::store::scabbard_store::{
        commit::{CommitEntryBuilder, ConsensusDecision},
        context::{ContextBuilder, Participant},
        service::{ConsensusType, ScabbardServiceBuilder, ServiceStatus},
        state::Scabbard2pcState,
        two_phase::{
            action::{ConsensusAction, ConsensusActionNotification},
            event::Scabbard2pcEvent,
            message::Scabbard2pcMessage,
        },
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

        let alarm = SystemTime::now()
            .checked_add(Duration::from_secs(60))
            .expect("failed to get alarm time");

        let coordinator_context = ContextBuilder::default()
            .with_alarm(alarm)
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.clone(),
                vote: None,
            }])
            .with_state(Scabbard2pcState::WaitingForStart)
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");
        let context = ScabbardContext::Scabbard2pcContext(coordinator_context);

        assert!(store
            .add_consensus_context(&coordinator_fqsi, context)
            .is_ok());

        let coordinator_context = ContextBuilder::default()
            .with_alarm(alarm)
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.clone(),
                vote: None,
            }])
            .with_state(Scabbard2pcState::WaitingForStart)
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");
        let context = ScabbardContext::Scabbard2pcContext(coordinator_context);

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

        let alarm = SystemTime::now()
            .checked_add(Duration::from_secs(60))
            .expect("failed to get alarm time");

        let coordinator_context = ContextBuilder::default()
            .with_alarm(alarm)
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.clone(),
                vote: None,
            }])
            .with_state(Scabbard2pcState::WaitingForStart)
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");
        let context = ScabbardContext::Scabbard2pcContext(coordinator_context);

        store
            .add_consensus_context(&coordinator_fqsi, context)
            .expect("failed to add context");

        let notification = ConsensusActionNotification::RequestForStart();
        let action = ScabbardConsensusAction::Scabbard2pcConsensusAction(ConsensusAction::Notify(
            notification,
        ));

        assert!(store
            .add_consensus_action(action, &coordinator_fqsi, 1)
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

        let coordinator_context = ContextBuilder::default()
            .with_alarm(alarm)
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.clone(),
                vote: None,
            }])
            .with_state(Scabbard2pcState::WaitingForStart)
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");
        let context = ScabbardContext::Scabbard2pcContext(coordinator_context);

        store
            .add_consensus_context(&coordinator_fqsi, context)
            .expect("failed to add context");

        let notification = ConsensusActionNotification::RequestForStart();
        let action1 = ScabbardConsensusAction::Scabbard2pcConsensusAction(ConsensusAction::Notify(
            notification,
        ));

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
            .with_alarm(alarm)
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.clone(),
                vote: None,
            }])
            .with_state(Scabbard2pcState::Voting { vote_timeout_start })
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build update context");

        let action2 = ScabbardConsensusAction::Scabbard2pcConsensusAction(ConsensusAction::Update(
            ScabbardContext::Scabbard2pcContext(update_context),
            None,
        ));

        store
            .add_consensus_action(action1, &coordinator_fqsi, 1)
            .expect("failed to add actions");
        store
            .add_consensus_action(action2, &coordinator_fqsi, 1)
            .expect("failed to add actions");

        let action_list = store
            .list_consensus_actions(&coordinator_fqsi, 1)
            .expect("failed to list all actions");

        let expected_update_context = ContextBuilder::default()
            .with_alarm(alarm)
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.clone(),
                vote: None,
            }])
            .with_state(Scabbard2pcState::Voting { vote_timeout_start })
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build update context");

        assert!(matches!(
            action_list[0],
            ScabbardConsensusAction::Scabbard2pcConsensusAction(ConsensusAction::Notify(
                ConsensusActionNotification::RequestForStart()
            ))
        ));
        assert!(matches!(
            &action_list[1],
            ScabbardConsensusAction::Scabbard2pcConsensusAction(ConsensusAction::Update(..))
        ));
        assert_eq!(
            action_list[1],
            ScabbardConsensusAction::Scabbard2pcConsensusAction(ConsensusAction::Update(
                ScabbardContext::Scabbard2pcContext(expected_update_context),
                None,
            ))
        );
    }

    /// Test that the scabbard store `update_consensus_context` operation is successful.
    ///
    /// 1. Add a valid context to the store
    /// 2. Create a valid updated context
    /// 3. Attempt to update the original context, check that the operation is successful
    /// 4. Create an invalid updated context
    /// 5. Attempt to update the original context, check that the operation returns an error
    #[test]
    fn scabbard_store_update_context() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselScabbardStore::new(pool);

        let coordinator_fqsi = FullyQualifiedServiceId::new_from_string("abcde-fghij::aa00")
            .expect("creating FullyQualifiedServiceId from string 'abcde-fghij::aa00'");

        let participant_service_id = ServiceId::new_random();

        let alarm = SystemTime::now()
            .checked_add(Duration::from_secs(60))
            .expect("failed to get alarm time");

        let coordinator_context = ContextBuilder::default()
            .with_alarm(alarm)
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.clone(),
                vote: None,
            }])
            .with_state(Scabbard2pcState::WaitingForStart)
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");
        let context = ScabbardContext::Scabbard2pcContext(coordinator_context);

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
            .with_alarm(alarm)
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.clone(),
                vote: None,
            }])
            .with_state(Scabbard2pcState::Voting { vote_timeout_start })
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build update context");

        assert!(store
            .update_consensus_context(
                &coordinator_fqsi,
                ScabbardContext::Scabbard2pcContext(update_context)
            )
            .is_ok());

        let bad_update_context = ContextBuilder::default()
            .with_alarm(alarm)
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(0)
            .with_participants(vec![Participant {
                process: participant_service_id.clone(),
                vote: None,
            }])
            .with_state(Scabbard2pcState::Voting { vote_timeout_start })
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build update context");

        assert!(store
            .update_consensus_context(
                &coordinator_fqsi,
                ScabbardContext::Scabbard2pcContext(bad_update_context)
            )
            .is_err());
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

        let alarm = SystemTime::now()
            .checked_add(Duration::from_secs(60))
            .expect("failed to get alarm time");

        let coordinator_context = ContextBuilder::default()
            .with_alarm(alarm)
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.clone(),
                vote: None,
            }])
            .with_state(Scabbard2pcState::WaitingForStart)
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");
        let context = ScabbardContext::Scabbard2pcContext(coordinator_context);

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
            .with_alarm(alarm)
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: participant_service_id.clone(),
                vote: None,
            }])
            .with_state(Scabbard2pcState::Voting { vote_timeout_start })
            .with_this_process(coordinator_fqsi.clone().service_id())
            .build()
            .expect("failed to build update context");
        let action = ScabbardConsensusAction::Scabbard2pcConsensusAction(ConsensusAction::Update(
            ScabbardContext::Scabbard2pcContext(update_context),
            None,
        ));

        let action_id = store
            .add_consensus_action(action, &coordinator_fqsi, 1)
            .expect("failed to add actions");

        assert!(store
            .update_consensus_action(&coordinator_fqsi, 1, action_id, SystemTime::now())
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
            .with_alarm(SystemTime::now())
            .with_coordinator(service_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: peer_service_id.clone(),
                vote: None,
            }])
            .with_state(Scabbard2pcState::WaitingForStart)
            .with_this_process(service_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");

        let context = ScabbardContext::Scabbard2pcContext(coordinator_context);

        store
            .add_consensus_context(&service_fqsi, context)
            .expect("failed to add context to store");

        let ready_services = store
            .list_ready_services()
            .expect("failed to list ready services");

        // check that the one service is returned because its alarm has passed
        assert_eq!(&ready_services[0], &service_fqsi);

        let notification = ConsensusActionNotification::RequestForStart();
        let action = ScabbardConsensusAction::Scabbard2pcConsensusAction(ConsensusAction::Notify(
            notification,
        ));

        let updated_alarm = SystemTime::now()
            .checked_add(Duration::from_secs(604800))
            .expect("failed to get alarm time");

        let update_context = ContextBuilder::default()
            .with_alarm(updated_alarm)
            .with_coordinator(service_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: peer_service_id.clone(),
                vote: None,
            }])
            .with_state(Scabbard2pcState::WaitingForStart)
            .with_this_process(service_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");

        // update the context to have an alarm far in the future
        store
            .update_consensus_context(
                &service_fqsi,
                ScabbardContext::Scabbard2pcContext(update_context),
            )
            .expect("failed to update context");

        // add an action for the service
        let action_id = store
            .add_consensus_action(action, &service_fqsi, 1)
            .expect("failed to add actions");

        let ready_services = store
            .list_ready_services()
            .expect("failed to list ready services");

        // check that the one service is still returned because it has an unexecuted action
        assert_eq!(&ready_services[0], &service_fqsi);

        store
            .update_consensus_action(&service_fqsi, 1, action_id, SystemTime::now())
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
            .with_alarm(SystemTime::now())
            .with_coordinator(service_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participants(vec![Participant {
                process: peer_service_id.clone(),
                vote: None,
            }])
            .with_state(Scabbard2pcState::WaitingForStart)
            .with_this_process(service_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");

        let context = ScabbardContext::Scabbard2pcContext(coordinator_context);

        store
            .add_consensus_context(&service_fqsi, context)
            .expect("failed to add context to store");

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
    /// 3. Attempt to add a coordinator specific event and check that an error is returned
    #[test]
    fn scabbard_store_add_event() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselScabbardStore::new(pool);

        let coordinator_fqsi = FullyQualifiedServiceId::new_random();

        let participant_fqsi = FullyQualifiedServiceId::new_random();
        let participant2_fqsi = FullyQualifiedServiceId::new_random();

        let alarm = SystemTime::now()
            .checked_add(Duration::from_secs(60))
            .expect("failed to get alarm time");

        let participant_context = ContextBuilder::default()
            .with_alarm(alarm)
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participant_processes(vec![
                participant_fqsi.service_id().clone(),
                participant2_fqsi.service_id().clone(),
            ])
            .with_state(Scabbard2pcState::WaitingForVoteRequest)
            .with_this_process(participant_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");
        let context = ScabbardContext::Scabbard2pcContext(participant_context);

        store
            .add_consensus_context(&participant_fqsi, context)
            .expect("failed to add context");

        let event = ScabbardConsensusEvent::Scabbard2pcConsensusEvent(Scabbard2pcEvent::Deliver(
            participant2_fqsi.service_id().clone(),
            Scabbard2pcMessage::DecisionRequest(1),
        ));

        assert!(store
            .add_consensus_event(&participant_fqsi, 1, event)
            .is_ok());

        let bad_event = ScabbardConsensusEvent::Scabbard2pcConsensusEvent(Scabbard2pcEvent::Start(
            vec![1].into(),
        ));

        assert!(store
            .add_consensus_event(&participant_fqsi, 1, bad_event)
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

        let alarm = SystemTime::now()
            .checked_add(Duration::from_secs(60))
            .expect("failed to get alarm time");

        let participant_context = ContextBuilder::default()
            .with_alarm(alarm)
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participant_processes(vec![
                participant_fqsi.service_id().clone(),
                participant2_fqsi.service_id().clone(),
            ])
            .with_state(Scabbard2pcState::WaitingForVoteRequest)
            .with_this_process(participant_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");
        let context = ScabbardContext::Scabbard2pcContext(participant_context);

        store
            .add_consensus_context(&participant_fqsi, context)
            .expect("failed to add context");

        let event = ScabbardConsensusEvent::Scabbard2pcConsensusEvent(Scabbard2pcEvent::Deliver(
            participant2_fqsi.service_id().clone(),
            Scabbard2pcMessage::DecisionRequest(1),
        ));

        let event_id = store
            .add_consensus_event(&participant_fqsi, 1, event)
            .expect("failed to add event");

        assert!(store
            .update_consensus_event(&participant_fqsi, 1, event_id, SystemTime::now())
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

        let alarm = SystemTime::now()
            .checked_add(Duration::from_secs(60))
            .expect("failed to get alarm time");

        let participant_context = ContextBuilder::default()
            .with_alarm(alarm)
            .with_coordinator(coordinator_fqsi.clone().service_id())
            .with_epoch(1)
            .with_participant_processes(vec![
                participant_fqsi.service_id().clone(),
                participant2_fqsi.service_id().clone(),
            ])
            .with_state(Scabbard2pcState::WaitingForVoteRequest)
            .with_this_process(participant_fqsi.clone().service_id())
            .build()
            .expect("failed to build context");
        let context = ScabbardContext::Scabbard2pcContext(participant_context);

        store
            .add_consensus_context(&participant_fqsi, context)
            .expect("failed to add context");

        let event = ScabbardConsensusEvent::Scabbard2pcConsensusEvent(Scabbard2pcEvent::Deliver(
            participant2_fqsi.service_id().clone(),
            Scabbard2pcMessage::DecisionRequest(1),
        ));

        let event_id = store
            .add_consensus_event(&participant_fqsi, 1, event)
            .expect("failed to add event");

        let events = store
            .list_consensus_events(&participant_fqsi, 1)
            .expect("failed to list events");

        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            ReturnedScabbardConsensusEvent::Scabbard2pcConsensusEvent(
                event_id,
                Scabbard2pcEvent::Deliver(
                    participant2_fqsi.service_id().clone(),
                    Scabbard2pcMessage::DecisionRequest(1)
                )
            )
        );

        let event2 = ScabbardConsensusEvent::Scabbard2pcConsensusEvent(Scabbard2pcEvent::Alarm());

        let event_id2 = store
            .add_consensus_event(&participant_fqsi, 1, event2)
            .expect("failed to add event");

        let events = store
            .list_consensus_events(&participant_fqsi, 1)
            .expect("failed to list events");

        assert_eq!(events.len(), 2);
        assert_eq!(
            events[0],
            ReturnedScabbardConsensusEvent::Scabbard2pcConsensusEvent(
                event_id,
                Scabbard2pcEvent::Deliver(
                    participant2_fqsi.service_id().clone(),
                    Scabbard2pcMessage::DecisionRequest(1)
                )
            ),
        );
        assert_eq!(
            events[1],
            ReturnedScabbardConsensusEvent::Scabbard2pcConsensusEvent(
                event_id2,
                Scabbard2pcEvent::Alarm()
            ),
        );

        store
            .update_consensus_event(&participant_fqsi, 1, event_id, SystemTime::now())
            .expect("failed to update event");

        let events = store
            .list_consensus_events(&participant_fqsi, 1)
            .expect("failed to list events");

        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            ReturnedScabbardConsensusEvent::Scabbard2pcConsensusEvent(
                event_id2,
                Scabbard2pcEvent::Alarm()
            ),
        );
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
