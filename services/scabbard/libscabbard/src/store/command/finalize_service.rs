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

use std::sync::Arc;
use std::time::SystemTime;

use splinter::{
    error::InternalError, service::FullyQualifiedServiceId, store::command::StoreCommand,
};

use crate::store::{context::ConsensusContext, service::ServiceStatus, ScabbardStoreFactory};

pub struct ScabbardFinalizeServiceCommand<C> {
    store_factory: Arc<dyn ScabbardStoreFactory<C>>,
    service_id: FullyQualifiedServiceId,
}

impl<C> ScabbardFinalizeServiceCommand<C> {
    pub fn new(
        store_factory: Arc<dyn ScabbardStoreFactory<C>>,
        service_id: FullyQualifiedServiceId,
    ) -> Self {
        Self {
            store_factory,
            service_id,
        }
    }
}

impl<C> StoreCommand for ScabbardFinalizeServiceCommand<C> {
    type Context = C;

    fn execute(&self, conn: &Self::Context) -> Result<(), InternalError> {
        let store = self.store_factory.new_store(conn);

        let service = store
            .get_service(&self.service_id)
            .map_err(|err| InternalError::from_source(Box::new(err)))?
            .ok_or_else(|| {
                InternalError::with_message(format!("Unable to fetch service {}", self.service_id))
            })?
            .into_builder()
            .with_status(&ServiceStatus::Finalized)
            .build()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        // set alarm to current time so it will be considered passed next time the timer runs
        let alarm = SystemTime::now();

        let mut context = store
            .get_current_consensus_context(&self.service_id)
            .map_err(|err| InternalError::from_source(Box::new(err)))?
            .ok_or_else(|| {
                InternalError::with_message(format!("No context found for {}", self.service_id))
            })?;

        context = match context {
            ConsensusContext::TwoPhaseCommit(context) => ConsensusContext::TwoPhaseCommit(
                context
                    .into_builder()
                    .with_alarm(alarm)
                    .build()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?,
            ),
        };

        store
            .update_service(service)
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        store
            .update_consensus_context(&self.service_id, context)
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        Ok(())
    }
}
