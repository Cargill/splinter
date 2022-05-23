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

use std::collections::HashMap;
use std::sync::Arc;

use augrim::Algorithm;
use splinter::error::InvalidStateError;
use splinter::service::MessageSenderFactory;
use splinter::store::command::StoreCommandExecutor;

use crate::service::v3::consensus::consensus_action_runner::{
    NotifyObserver, ScabbardStoreContextUpdater,
};
use crate::store::ConsensusAction;
use crate::store::ConsensusEvent;
use crate::store::{PooledScabbardStoreFactory, ScabbardStoreFactory};

use super::{
    ConsensusActionRunner, ConsensusContext, ConsensusRunner, ConsensusStoreCommandFactory,
};

#[derive(Default)]
pub struct ConsensusRunnerBuilder<E>
where
    E: StoreCommandExecutor,
    <E as StoreCommandExecutor>::Context: Sized,
{
    pooled_scabbard_store_factory: Option<Arc<dyn PooledScabbardStoreFactory>>,
    algorithms: HashMap<
        String,
        Box<
            dyn Algorithm<
                Event = ConsensusEvent,
                Action = ConsensusAction,
                Context = ConsensusContext,
            >,
        >,
    >,
    scabbard_store_factory:
        Option<Arc<dyn ScabbardStoreFactory<<E as StoreCommandExecutor>::Context>>>,
    store_command_executor: Option<Arc<E>>,
    message_sender_factory: Option<Box<dyn MessageSenderFactory<Vec<u8>>>>,
    notify_observer: Option<Box<dyn NotifyObserver<<E as StoreCommandExecutor>::Context>>>,
}

impl<E> ConsensusRunnerBuilder<E>
where
    E: StoreCommandExecutor,
    <E as StoreCommandExecutor>::Context: Sized,
{
    pub fn new() -> Self {
        Self {
            pooled_scabbard_store_factory: None,
            algorithms: HashMap::new(),
            scabbard_store_factory: None,
            store_command_executor: None,
            message_sender_factory: None,
            notify_observer: None,
        }
    }

    pub fn with_pooled_scabbard_store_factory(
        mut self,
        factory: Arc<dyn PooledScabbardStoreFactory>,
    ) -> Self {
        self.pooled_scabbard_store_factory = Some(factory);
        self
    }

    pub fn with_scabbard_store_factory(
        mut self,
        factory: Arc<dyn ScabbardStoreFactory<<E as StoreCommandExecutor>::Context>>,
    ) -> Self {
        self.scabbard_store_factory = Some(factory);
        self
    }

    pub fn with_store_command_executor(mut self, store_command_executor: Arc<E>) -> Self {
        self.store_command_executor = Some(store_command_executor);
        self
    }

    pub fn with_algorithm<S: Into<String>>(
        mut self,
        algorithm_name: S,
        algorithm: Box<
            dyn Algorithm<
                Event = ConsensusEvent,
                Action = ConsensusAction,
                Context = ConsensusContext,
            >,
        >,
    ) -> Self {
        self.algorithms.insert(algorithm_name.into(), algorithm);
        self
    }

    pub fn with_message_sender_factory(
        mut self,
        message_sender_factory: Box<dyn MessageSenderFactory<Vec<u8>>>,
    ) -> Self {
        self.message_sender_factory = Some(message_sender_factory);
        self
    }

    pub fn with_notify_observer(
        mut self,
        notify_observer: Box<dyn NotifyObserver<<E as StoreCommandExecutor>::Context>>,
    ) -> Self {
        self.notify_observer = Some(notify_observer);
        self
    }

    pub fn build(self) -> Result<ConsensusRunner<E>, InvalidStateError> {
        let pooled_scabbard_store_factory =
            self.pooled_scabbard_store_factory.ok_or_else(|| {
                InvalidStateError::with_message(
                    "A pooled_scabbard_store_factory must be provided".into(),
                )
            })?;

        let scabbard_store_factory = self.scabbard_store_factory.ok_or_else(|| {
            InvalidStateError::with_message("A scabbard_store_factory must be provided".into())
        })?;

        let store_command_executor = self.store_command_executor.ok_or_else(|| {
            InvalidStateError::with_message("A store_command_executor must be provided".into())
        })?;

        let message_sender_factory = self.message_sender_factory.ok_or_else(|| {
            InvalidStateError::with_message("A message_sender_factory must be provided".into())
        })?;

        let notify_observer = self.notify_observer.ok_or_else(|| {
            InvalidStateError::with_message("A notify_observer must be provided".into())
        })?;

        let consensus_store_command_factory =
            ConsensusStoreCommandFactory::new(scabbard_store_factory.clone());

        Ok(ConsensusRunner {
            pooled_scabbard_store_factory,
            action_runner: ConsensusActionRunner::new(
                message_sender_factory,
                Box::new(ScabbardStoreContextUpdater::new(
                    scabbard_store_factory.clone(),
                )),
                notify_observer,
                scabbard_store_factory,
            ),
            algorithms: self.algorithms,
            consensus_store_command_factory,
            store_command_executor,
        })
    }
}
