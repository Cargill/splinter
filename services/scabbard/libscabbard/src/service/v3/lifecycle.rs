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

use std::marker::PhantomData;

use splinter::{
    error::InternalError,
    service::{FullyQualifiedServiceId, Lifecycle},
    store::command::StoreCommand,
};

use crate::store::{
    ScabbardFinalizeServiceCommand, ScabbardPrepareServiceCommand, ScabbardPurgeServiceCommand,
    ScabbardRetireServiceCommand,
};

use super::ScabbardArguments;

#[derive(Default)]
pub struct ScabbardLifecycle<K> {
    _store_factory: PhantomData<K>,
}

impl<K> ScabbardLifecycle<K> {
    pub fn new() -> Self {
        Self {
            _store_factory: PhantomData,
        }
    }
}

impl<K> Lifecycle<K> for ScabbardLifecycle<K>
where
    K: 'static,
{
    type Arguments = ScabbardArguments;

    fn command_to_prepare(
        &self,
        _service: FullyQualifiedServiceId,
        _arguments: Self::Arguments,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
        Ok(Box::new(ScabbardPrepareServiceCommand::new()))
    }

    fn command_to_finalize(
        &self,
        _service: FullyQualifiedServiceId,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
        Ok(Box::new(ScabbardFinalizeServiceCommand::new()))
    }

    fn command_to_retire(
        &self,
        _service: FullyQualifiedServiceId,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
        Ok(Box::new(ScabbardRetireServiceCommand::new()))
    }

    fn command_to_purge(
        &self,
        _service: FullyQualifiedServiceId,
    ) -> Result<Box<dyn StoreCommand<Context = K>>, InternalError> {
        Ok(Box::new(ScabbardPurgeServiceCommand::new()))
    }
}
