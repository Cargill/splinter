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

use splinter::error::{InternalError, InvalidArgumentError};
use splinter::service::{TimerHandler, TimerHandlerFactory};

use crate::store::PooledScabbardStoreFactory;

use super::ScabbardMessageByteConverter;
use super::ScabbardTimerHandler;

#[derive(Clone)]
pub struct ScabbardTimerHandlerFactory {
    store_factory: Box<dyn PooledScabbardStoreFactory>,
}

impl ScabbardTimerHandlerFactory {
    pub fn store_factory(&self) -> &dyn PooledScabbardStoreFactory {
        &*self.store_factory
    }
}

impl TimerHandlerFactory for ScabbardTimerHandlerFactory {
    type Message = Vec<u8>;

    fn new_handler(&self) -> Result<Box<dyn TimerHandler<Message = Self::Message>>, InternalError> {
        let timer_handler = ScabbardTimerHandler::new();
        Ok(Box::new(
            timer_handler.into_handler(ScabbardMessageByteConverter {}),
        ))
    }

    fn clone_box(&self) -> Box<dyn TimerHandlerFactory<Message = Self::Message>> {
        Box::new(self.clone())
    }
}

#[derive(Default)]
pub struct ScabbardTimerHandlerFactoryBuilder {
    store_factory: Option<Box<dyn PooledScabbardStoreFactory>>,
}

impl ScabbardTimerHandlerFactoryBuilder {
    pub fn with_store_factory(
        mut self,
        store_factory: Box<dyn PooledScabbardStoreFactory>,
    ) -> Self {
        self.store_factory = Some(store_factory);
        self
    }

    pub fn build(self) -> Result<ScabbardTimerHandlerFactory, InvalidArgumentError> {
        let store_factory = self
            .store_factory
            .ok_or_else(|| InvalidArgumentError::new("store_factory", "must be set"))?;

        Ok(ScabbardTimerHandlerFactory { store_factory })
    }
}
