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

//! Contains an implementation of a `ContextUpdater` that soley uses commands to update
//! the context and alarms.

use std::sync::Arc;
use std::time::SystemTime;

use splinter::error::InternalError;
use splinter::service::FullyQualifiedServiceId;
use splinter::store::command::StoreCommand;

use crate::service::v3::consensus::consensus_action_runner::UpdateContextCommand;
use crate::store::ConsensusContext;
use crate::store::ScabbardStoreFactory;

use super::ContextUpdater;

/// Implementation of a `ContextUpdater` that uses the `ScabbardStore` and commands
pub struct ScabbardStoreContextUpdater<C: 'static> {
    store_factory: Arc<dyn ScabbardStoreFactory<C>>,
}

impl<C> ScabbardStoreContextUpdater<C> {
    // Create a new `ScabbardStoreContextUpdater`
    ///
    /// # Arguments
    ///
    /// * `store_factory` - The scabbard store factory to be used by commands
    pub fn new(store_factory: Arc<dyn ScabbardStoreFactory<C>>) -> Self {
        ScabbardStoreContextUpdater { store_factory }
    }
}

impl<C: 'static> ContextUpdater<C> for ScabbardStoreContextUpdater<C> {
    /// Update context and alarms
    ///
    /// # Arguments
    ///
    /// * `context` - The context to be updated
    /// * `service_id` - The service ID of of the service the notification is for
    /// * `alarm` - The alarm to update
    fn update(
        &self,
        context: ConsensusContext,
        service_id: &FullyQualifiedServiceId,
        alarm: Option<SystemTime>,
    ) -> Result<Vec<Box<dyn StoreCommand<Context = C>>>, InternalError> {
        Ok(vec![Box::new(UpdateContextCommand::new(
            context,
            service_id.clone(),
            alarm,
            self.store_factory.clone(),
        ))])
    }
}
