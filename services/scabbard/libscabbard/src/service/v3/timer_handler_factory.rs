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

use augrim::two_phase_commit::TwoPhaseCommitAlgorithm;
use augrim::Algorithm;
use splinter::error::{InternalError, InvalidArgumentError};
use splinter::service::{MessageSenderFactory, TimerHandler, TimerHandlerFactory};
use splinter::store::command::StoreCommandExecutor;

use crate::store::{PooledScabbardStoreFactory, ScabbardStoreFactory};

use super::ScabbardMessageByteConverter;
use super::ScabbardTimerHandler;
use super::{CommandNotifyObserver, ConsensusRunnerBuilder};

#[derive(Clone)]
pub struct ScabbardTimerHandlerFactory<E>
where
    E: StoreCommandExecutor + 'static,
{
    pooled_store_factory: Box<dyn PooledScabbardStoreFactory>,
    store_factory: Arc<dyn ScabbardStoreFactory<<E as StoreCommandExecutor>::Context>>,
    message_sender_factory: Box<dyn MessageSenderFactory<Vec<u8>>>,
    store_command_executor: Arc<E>,
}

impl<E: StoreCommandExecutor + 'static> ScabbardTimerHandlerFactory<E> {
    pub fn pooled_store_factory(&self) -> &dyn PooledScabbardStoreFactory {
        &*self.pooled_store_factory
    }
}

impl<E: StoreCommandExecutor + 'static> TimerHandlerFactory for ScabbardTimerHandlerFactory<E> {
    type Message = Vec<u8>;

    fn new_handler(&self) -> Result<Box<dyn TimerHandler<Message = Self::Message>>, InternalError> {
        let consensus_runner = ConsensusRunnerBuilder::new()
            .with_store_command_executor(self.store_command_executor.clone())
            .with_scabbard_store_factory(self.store_factory.clone())
            .with_pooled_scabbard_store_factory(self.pooled_store_factory.clone().into())
            .with_message_sender_factory(self.message_sender_factory.clone())
            .with_notify_observer(Box::new(CommandNotifyObserver::new(
                self.store_factory.clone(),
                self.pooled_store_factory.new_store(),
            )))
            .with_algorithm(
                "two-phase-commit",
                Box::new(
                    TwoPhaseCommitAlgorithm::new(augrim::SystemTimeFactory::new()).into_algorithm(),
                ),
            )
            .build()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let timer_handler =
            ScabbardTimerHandler::new(consensus_runner, self.pooled_store_factory.new_store());
        Ok(Box::new(
            timer_handler.into_handler(ScabbardMessageByteConverter {}),
        ))
    }

    fn clone_box(&self) -> Box<dyn TimerHandlerFactory<Message = Self::Message>> {
        Box::new({
            Self {
                pooled_store_factory: self.pooled_store_factory.clone(),
                store_factory: self.store_factory.clone(),
                message_sender_factory: self.message_sender_factory.clone(),
                store_command_executor: self.store_command_executor.clone(),
            }
        })
    }
}

#[derive(Default)]
pub struct ScabbardTimerHandlerFactoryBuilder<E>
where
    E: StoreCommandExecutor + 'static,
{
    pooled_store_factory: Option<Box<dyn PooledScabbardStoreFactory>>,
    store_factory: Option<Arc<dyn ScabbardStoreFactory<<E as StoreCommandExecutor>::Context>>>,
    message_sender_factory: Option<Box<dyn MessageSenderFactory<Vec<u8>>>>,
    store_command_executor: Option<Arc<E>>,
}

impl<E: StoreCommandExecutor + 'static> ScabbardTimerHandlerFactoryBuilder<E> {
    pub fn new() -> Self {
        Self {
            pooled_store_factory: None,
            store_factory: None,
            message_sender_factory: None,
            store_command_executor: None,
        }
    }

    pub fn with_pooled_store_factory(
        mut self,
        pooled_store_factory: Box<dyn PooledScabbardStoreFactory>,
    ) -> Self {
        self.pooled_store_factory = Some(pooled_store_factory);
        self
    }

    pub fn with_store_factory(
        mut self,
        store_factory: Arc<dyn ScabbardStoreFactory<<E as StoreCommandExecutor>::Context>>,
    ) -> Self {
        self.store_factory = Some(store_factory);
        self
    }

    pub fn with_message_sender_factory(
        mut self,
        message_sender_factory: Box<dyn MessageSenderFactory<Vec<u8>>>,
    ) -> Self {
        self.message_sender_factory = Some(message_sender_factory);
        self
    }

    pub fn with_store_command_executor(mut self, store_command_executor: Arc<E>) -> Self {
        self.store_command_executor = Some(store_command_executor);
        self
    }

    pub fn build(self) -> Result<ScabbardTimerHandlerFactory<E>, InvalidArgumentError> {
        let pooled_store_factory = self
            .pooled_store_factory
            .ok_or_else(|| InvalidArgumentError::new("pooled_store_factory", "must be set"))?;

        let store_factory = self
            .store_factory
            .ok_or_else(|| InvalidArgumentError::new("store_factory", "must be set"))?;

        let message_sender_factory = self
            .message_sender_factory
            .ok_or_else(|| InvalidArgumentError::new("message_sender_factory", "must be set"))?;

        let store_command_executor = self
            .store_command_executor
            .ok_or_else(|| InvalidArgumentError::new("store_command_executor", "must be set"))?;

        Ok(ScabbardTimerHandlerFactory {
            pooled_store_factory,
            store_factory,
            message_sender_factory,
            store_command_executor,
        })
    }
}
