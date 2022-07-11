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

//! Contains commands for marking notifications as handled

use std::sync::Arc;
use std::time::SystemTime;

use splinter::error::InternalError;
use splinter::service::FullyQualifiedServiceId;
use splinter::store::command::StoreCommand;

use crate::store::CommitEntry;
use crate::store::ConsensusEvent;
use crate::store::ScabbardStoreFactory;
use crate::store::SupervisorNotification;

pub struct AddEventCommand<C> {
    store_factory: Arc<dyn ScabbardStoreFactory<C>>,
    service_id: FullyQualifiedServiceId,
    event: ConsensusEvent,
}

impl<C> AddEventCommand<C> {
    pub fn new(
        store_factory: Arc<dyn ScabbardStoreFactory<C>>,
        service_id: FullyQualifiedServiceId,
        event: ConsensusEvent,
    ) -> Self {
        Self {
            store_factory,
            service_id,
            event,
        }
    }
}

impl<C> StoreCommand for AddEventCommand<C> {
    type Context = C;

    fn execute(&self, conn: &Self::Context) -> Result<(), InternalError> {
        self.store_factory
            .new_store(&*conn)
            .add_consensus_event(&self.service_id, self.event.clone())
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        Ok(())
    }
}

pub struct AddCommitEntryCommand<C> {
    store_factory: Arc<dyn ScabbardStoreFactory<C>>,
    commit_entry: CommitEntry,
}

impl<C> AddCommitEntryCommand<C> {
    pub fn new(store_factory: Arc<dyn ScabbardStoreFactory<C>>, commit_entry: CommitEntry) -> Self {
        Self {
            store_factory,
            commit_entry,
        }
    }
}

impl<C> StoreCommand for AddCommitEntryCommand<C> {
    type Context = C;

    fn execute(&self, conn: &Self::Context) -> Result<(), InternalError> {
        self.store_factory
            .new_store(&*conn)
            .add_commit_entry(self.commit_entry.clone())
            .map_err(|e| InternalError::from_source(Box::new(e)))
    }
}

pub struct UpdateCommitEntryCommand<C> {
    store_factory: Arc<dyn ScabbardStoreFactory<C>>,
    commit_entry: CommitEntry,
}

impl<C> UpdateCommitEntryCommand<C> {
    pub fn new(store_factory: Arc<dyn ScabbardStoreFactory<C>>, commit_entry: CommitEntry) -> Self {
        Self {
            store_factory,
            commit_entry,
        }
    }
}

impl<C> StoreCommand for UpdateCommitEntryCommand<C> {
    type Context = C;

    fn execute(&self, conn: &Self::Context) -> Result<(), InternalError> {
        self.store_factory
            .new_store(&*conn)
            .update_commit_entry(self.commit_entry.clone())
            .map_err(|e| InternalError::from_source(Box::new(e)))
    }
}

pub struct ExecuteSupervisorCommand<C> {
    store_factory: Arc<dyn ScabbardStoreFactory<C>>,
    service_id: FullyQualifiedServiceId,
    notification_id: i64,
}

impl<C> ExecuteSupervisorCommand<C> {
    pub fn new(
        store_factory: Arc<dyn ScabbardStoreFactory<C>>,
        service_id: FullyQualifiedServiceId,
        notification_id: i64,
    ) -> Self {
        Self {
            store_factory,
            service_id,
            notification_id,
        }
    }
}

impl<C> StoreCommand for ExecuteSupervisorCommand<C> {
    type Context = C;

    fn execute(&self, conn: &Self::Context) -> Result<(), InternalError> {
        self.store_factory
            .new_store(&*conn)
            .update_supervisor_notification(
                &self.service_id,
                self.notification_id,
                SystemTime::now(),
            )
            .map_err(|e| InternalError::from_source(Box::new(e)))
    }
}

pub struct AddSupervisorNotficationCommand<C> {
    store_factory: Arc<dyn ScabbardStoreFactory<C>>,
    notification: SupervisorNotification,
}

impl<C> AddSupervisorNotficationCommand<C> {
    pub fn new(
        store_factory: Arc<dyn ScabbardStoreFactory<C>>,
        notification: SupervisorNotification,
    ) -> Self {
        Self {
            store_factory,
            notification,
        }
    }
}

impl<C> StoreCommand for AddSupervisorNotficationCommand<C> {
    type Context = C;

    fn execute(&self, conn: &Self::Context) -> Result<(), InternalError> {
        self.store_factory
            .new_store(&*conn)
            .add_supervisor_notification(self.notification.clone())
            .map_err(|e| InternalError::from_source(Box::new(e)))
    }
}
