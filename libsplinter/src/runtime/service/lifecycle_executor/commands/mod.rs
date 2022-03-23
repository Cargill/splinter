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

//! Commands for updating the lifecycle status of a service
mod generator;

use std::sync::Arc;

use crate::error::InternalError;
use crate::runtime::service::{LifecycleService, LifecycleStoreFactory};
use crate::store::command::StoreCommand;

pub use self::generator::LifecycleCommandGenerator;

pub struct LifecycleCompleteCommand<C> {
    service: LifecycleService,
    store_factory: Arc<dyn LifecycleStoreFactory<C>>,
}

impl<C> StoreCommand for LifecycleCompleteCommand<C> {
    type Context = C;

    fn execute(&self, conn: &Self::Context) -> Result<(), InternalError> {
        self.store_factory
            .new_store(&*conn)
            .update_service(self.service.clone())
            .map_err(|e| InternalError::from_source(Box::new(e)))
    }
}

pub struct LifecycleRemoveCommand<C> {
    service: LifecycleService,
    store_factory: Arc<dyn LifecycleStoreFactory<C>>,
}

impl<C> StoreCommand for LifecycleRemoveCommand<C> {
    type Context = C;

    fn execute(&self, conn: &Self::Context) -> Result<(), InternalError> {
        self.store_factory
            .new_store(&*conn)
            .remove_service(self.service.service_id())
            .map_err(|e| InternalError::from_source(Box::new(e)))
    }
}
