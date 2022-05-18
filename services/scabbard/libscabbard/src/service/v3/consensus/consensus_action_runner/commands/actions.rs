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

//! Contains commands for marking actions as executed

use std::sync::Arc;
use std::time::SystemTime;

use splinter::error::InternalError;
use splinter::service::FullyQualifiedServiceId;
use splinter::store::command::StoreCommand;

use crate::store::ScabbardStoreFactory;

pub struct ExecuteActionCommand<C> {
    service_id: FullyQualifiedServiceId,
    action_id: i64,
    store_factory: Arc<dyn ScabbardStoreFactory<C>>,
}

impl<C> ExecuteActionCommand<C> {
    pub fn new(
        service_id: FullyQualifiedServiceId,
        action_id: i64,
        store_factory: Arc<dyn ScabbardStoreFactory<C>>,
    ) -> Self {
        Self {
            service_id,
            action_id,
            store_factory,
        }
    }
}

impl<C> StoreCommand for ExecuteActionCommand<C> {
    type Context = C;

    fn execute(&self, conn: &Self::Context) -> Result<(), InternalError> {
        self.store_factory
            .new_store(&*conn)
            .update_consensus_action(&self.service_id, self.action_id, SystemTime::now())
            .map_err(|e| InternalError::from_source(Box::new(e)))
    }
}
