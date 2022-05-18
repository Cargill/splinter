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

use std::cell::RefCell;
use std::sync::Arc;
use std::time::SystemTime;

use splinter::error::InternalError;
use splinter::service::FullyQualifiedServiceId;
use splinter::store::command::StoreCommand;

use crate::store::ConsensusAction;
use crate::store::ScabbardStoreFactory;

pub struct ConsensusStoreCommandFactory<C> {
    factory: Arc<dyn ScabbardStoreFactory<C>>,
}

impl<C> ConsensusStoreCommandFactory<C>
where
    C: 'static,
{
    pub fn new(factory: Arc<dyn ScabbardStoreFactory<C>>) -> Self {
        Self { factory }
    }

    pub fn new_save_actions_command(
        &self,
        service_id: &FullyQualifiedServiceId,
        actions: Vec<ConsensusAction>,
    ) -> Box<dyn StoreCommand<Context = C>> {
        Box::new(SaveActionsCommand {
            factory: self.factory.clone(),
            service_id: service_id.clone(),
            actions: RefCell::new(actions),
        })
    }

    pub fn new_mark_event_complete_command(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
        event_id: i64,
    ) -> Box<dyn StoreCommand<Context = C>> {
        Box::new(MarkEventCompleteCommand {
            factory: self.factory.clone(),
            service_id: service_id.clone(),
            epoch,
            event_id,
        })
    }
}

struct SaveActionsCommand<C> {
    factory: Arc<dyn ScabbardStoreFactory<C>>,
    service_id: FullyQualifiedServiceId,
    actions: RefCell<Vec<ConsensusAction>>,
}

impl<C> StoreCommand for SaveActionsCommand<C>
where
    C: 'static,
{
    type Context = C;

    fn execute(&self, conn: &Self::Context) -> Result<(), InternalError> {
        let store = self.factory.new_store(conn);

        for action in self.actions.borrow_mut().drain(..) {
            store
                .add_consensus_action(action, &self.service_id)
                .map_err(|e| InternalError::from_source(Box::new(e)))?;
        }

        Ok(())
    }
}

struct MarkEventCompleteCommand<C> {
    factory: Arc<dyn ScabbardStoreFactory<C>>,
    service_id: FullyQualifiedServiceId,
    epoch: u64,
    event_id: i64,
}

impl<C> StoreCommand for MarkEventCompleteCommand<C>
where
    C: 'static,
{
    type Context = C;

    fn execute(&self, conn: &Self::Context) -> Result<(), InternalError> {
        let store = self.factory.new_store(conn);
        store
            .update_consensus_event(
                &self.service_id,
                self.epoch,
                self.event_id,
                SystemTime::now(),
            )
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        Ok(())
    }
}
