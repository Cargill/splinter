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

use crate::error::InternalError;
use crate::runtime::service::{
    LifecycleCommand, LifecycleService, LifecycleStatus, LifecycleStoreFactory,
};
use crate::store::command::StoreCommand;

use super::{LifecycleCompleteCommand, LifecycleRemoveCommand};

pub struct LifecycleCommandGenerator<C: 'static> {
    store_factory: Arc<dyn LifecycleStoreFactory<C>>,
}

impl<C: 'static> LifecycleCommandGenerator<C> {
    pub fn new(store_factory: Arc<dyn LifecycleStoreFactory<C>>) -> Self {
        LifecycleCommandGenerator { store_factory }
    }

    pub fn complete_service(
        &self,
        service: LifecycleService,
    ) -> Result<Box<dyn StoreCommand<Context = C>>, InternalError> {
        match service.command() {
            LifecycleCommand::Purge => Ok(Box::new(LifecycleRemoveCommand {
                service,
                store_factory: self.store_factory.clone(),
            })),
            _ => {
                let service = service
                    .into_builder()
                    .with_status(&LifecycleStatus::Complete)
                    .build()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;

                Ok(Box::new(LifecycleCompleteCommand {
                    service,
                    store_factory: self.store_factory.clone(),
                }))
            }
        }
    }
}
