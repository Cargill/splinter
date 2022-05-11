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

//! Contains commands for updating contexts and alarms

use std::sync::Arc;

use splinter::error::InternalError;
use splinter::service::FullyQualifiedServiceId;
use splinter::store::command::StoreCommand;

use crate::store::context::ConsensusContext;
use crate::store::ScabbardStoreFactory;

pub struct UpdateContextCommand<C> {
    context: ConsensusContext,
    service_id: FullyQualifiedServiceId,
    store_factory: Arc<dyn ScabbardStoreFactory<C>>,
}

impl<C> UpdateContextCommand<C> {
    pub fn new(
        context: ConsensusContext,
        service_id: FullyQualifiedServiceId,
        store_factory: Arc<dyn ScabbardStoreFactory<C>>,
    ) -> Self {
        Self {
            context,
            service_id,
            store_factory,
        }
    }
}

impl<C> StoreCommand for UpdateContextCommand<C> {
    type Context = C;

    fn execute(&self, conn: &Self::Context) -> Result<(), InternalError> {
        self.store_factory
            .new_store(&*conn)
            .update_consensus_context(&self.service_id, self.context.clone())
            .map_err(|e| InternalError::from_source(Box::new(e)))
    }
}
