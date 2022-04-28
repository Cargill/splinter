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

use splinter::{
    error::InternalError,
    service::{FullyQualifiedServiceId, Lifecycle},
    store::command::StoreCommand,
};

use crate::store::{
    service::{ScabbardServiceBuilder, ServiceStatus},
    ScabbardFinalizeServiceCommand, ScabbardPrepareServiceCommand, ScabbardPurgeServiceCommand,
    ScabbardRetireServiceCommand, ScabbardStoreFactory,
};

use super::ScabbardArguments;

pub struct ScabbardLifecycle<K> {
    store_factory: Arc<dyn ScabbardStoreFactory<K>>,
}

impl<K> ScabbardLifecycle<K> {
    pub fn new(store_factory: Arc<dyn ScabbardStoreFactory<K>>) -> Self {
        Self { store_factory }
    }
}

impl<K> Lifecycle<K> for ScabbardLifecycle<K>
where
    K: 'static,
{
    type Arguments = ScabbardArguments;

    fn command_to_prepare(
        &self,
        service: FullyQualifiedServiceId,
        arguments: Self::Arguments,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
        let built_service = ScabbardServiceBuilder::default()
            .with_service_id(&service)
            .with_peers(arguments.peers())
            .with_consensus(arguments.consensus())
            .with_status(&ServiceStatus::Prepared)
            .build()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        Ok(Box::new(ScabbardPrepareServiceCommand::new(
            self.store_factory.clone(),
            built_service,
        )))
    }

    fn command_to_finalize(
        &self,
        service: FullyQualifiedServiceId,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
        Ok(Box::new(ScabbardFinalizeServiceCommand::new(
            self.store_factory.clone(),
            service,
        )))
    }

    fn command_to_retire(
        &self,
        service: FullyQualifiedServiceId,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
        Ok(Box::new(ScabbardRetireServiceCommand::new(
            self.store_factory.clone(),
            service,
        )))
    }

    fn command_to_purge(
        &self,
        service: FullyQualifiedServiceId,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
        Ok(Box::new(ScabbardPurgeServiceCommand::new(
            self.store_factory.clone(),
            service,
        )))
    }
}
