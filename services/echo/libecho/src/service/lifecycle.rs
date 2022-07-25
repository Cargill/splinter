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
    EchoFinalizeServiceCommand, EchoPrepareServiceCommand, EchoPurgeServiceCommand,
    EchoRetireServiceCommand, EchoStoreFactory,
};

use super::EchoArguments;

pub struct EchoLifecycle<K> {
    store_factory: Arc<dyn EchoStoreFactory<K>>,
}

impl<K> EchoLifecycle<K> {
    pub fn new(store_factory: Arc<dyn EchoStoreFactory<K>>) -> Self {
        EchoLifecycle { store_factory }
    }
}

impl<K> Lifecycle<K> for EchoLifecycle<K>
where
    K: 'static,
{
    type Arguments = EchoArguments;

    fn command_to_prepare(
        &self,
        service: FullyQualifiedServiceId,
        arguments: Self::Arguments,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
        Ok(Box::new(EchoPrepareServiceCommand::new(
            self.store_factory.clone(),
            service,
            arguments,
        )))
    }

    fn command_to_finalize(
        &self,
        service: FullyQualifiedServiceId,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
        Ok(Box::new(EchoFinalizeServiceCommand::new(
            self.store_factory.clone(),
            service,
        )))
    }

    fn command_to_retire(
        &self,
        service: FullyQualifiedServiceId,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
        Ok(Box::new(EchoRetireServiceCommand::new(
            self.store_factory.clone(),
            service,
        )))
    }

    fn command_to_purge(
        &self,
        service: FullyQualifiedServiceId,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
        Ok(Box::new(EchoPurgeServiceCommand::new(
            self.store_factory.clone(),
            service,
        )))
    }
}
