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

use std::time::Instant;

use splinter::error::{InternalError, InvalidArgumentError};
use splinter::service::{TimerHandler, TimerHandlerFactory};

use crate::store::PooledEchoStoreFactory;

use super::{EchoMessageByteConverter, EchoTimerHandler};

#[derive(Clone)]
pub struct EchoTimerHandlerFactory {
    store_factory: Box<dyn PooledEchoStoreFactory>,
}

impl EchoTimerHandlerFactory {
    pub fn store_factory(&self) -> &dyn PooledEchoStoreFactory {
        &*self.store_factory
    }
}

impl TimerHandlerFactory for EchoTimerHandlerFactory {
    type Message = Vec<u8>;

    fn new_handler(&self) -> Result<Box<dyn TimerHandler<Message = Self::Message>>, InternalError> {
        let timer_handler = EchoTimerHandler::new(self.store_factory.new_store(), Instant::now());
        Ok(Box::new(
            timer_handler.into_handler(EchoMessageByteConverter {}),
        ))
    }

    fn clone_box(&self) -> Box<dyn TimerHandlerFactory<Message = Self::Message>> {
        Box::new(self.clone())
    }
}

#[derive(Default)]
pub struct EchoTimerHandlerFactoryBuilder {
    store_factory: Option<Box<dyn PooledEchoStoreFactory>>,
}

impl EchoTimerHandlerFactoryBuilder {
    pub fn with_store_factory(mut self, store_factory: Box<dyn PooledEchoStoreFactory>) -> Self {
        self.store_factory = Some(store_factory);
        self
    }

    pub fn build(self) -> Result<EchoTimerHandlerFactory, InvalidArgumentError> {
        let store_factory = self
            .store_factory
            .ok_or_else(|| InvalidArgumentError::new("store_factory", "must be set"))?;

        Ok(EchoTimerHandlerFactory { store_factory })
    }
}
