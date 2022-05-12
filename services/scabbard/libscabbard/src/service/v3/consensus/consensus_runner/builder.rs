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
use crate::store::action::ConsensusAction;
use crate::store::event::ConsensusEvent;
use crate::store::ScabbardStoreFactory;

use super::{
    ConsensusActionRunner, ConsensusContext, ConsensusRunner, ConsensusStoreCommandFactory,
    ContextSource, ScabbardProcess, UnprocessedActionSource, UnprocessedEventSource,
};

#[derive(Default)]
pub struct ConsensusRunnerBuilder<E>
where
    E: StoreCommandExecutor,
    <E as StoreCommandExecutor>::Context: Sized,
{
    unprocessed_action_source: Option<Box<dyn UnprocessedActionSource>>,
    unprocessed_event_source: Option<Box<dyn UnprocessedEventSource>>,
    context_source: Option<Box<dyn ContextSource>>,
    algorithms: HashMap<
        String,
        Box<
            dyn Algorithm<
                ScabbardProcess,
                Event = ConsensusEvent,
                Action = ConsensusAction,
                Context = ConsensusContext,
            >,
        >,
    >,
    scabbard_store_factory:
        Option<Arc<dyn ScabbardStoreFactory<<E as StoreCommandExecutor>::Context>>>,
    store_command_executor: Option<E>,
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
            unprocessed_action_source: None,
            unprocessed_event_source: None,
            context_source: None,
            algorithms: HashMap::new(),
            scabbard_store_factory: None,
            store_command_executor: None,
            message_sender_factory: None,
            notify_observer: None,
        }
    }

    pub fn with_unprocessed_action_source(
        mut self,
        unprocessed_action_source: Box<dyn UnprocessedActionSource>,
    ) -> Self {
        self.unprocessed_action_source = Some(unprocessed_action_source);
        self
    }

    pub fn with_unprocessed_event_source(
        mut self,
        unprocessed_event_source: Box<dyn UnprocessedEventSource>,
    ) -> Self {
        self.unprocessed_event_source = Some(unprocessed_event_source);
        self
    }

    pub fn with_context_source(mut self, context_source: Box<dyn ContextSource>) -> Self {
        self.context_source = Some(context_source);
        self
    }

    pub fn with_scabbard_store_factory(
        mut self,
        factory: Arc<dyn ScabbardStoreFactory<<E as StoreCommandExecutor>::Context>>,
    ) -> Self {
        self.scabbard_store_factory = Some(factory);
        self
    }

    pub fn with_store_command_executor(mut self, store_command_executor: E) -> Self {
        self.store_command_executor = Some(store_command_executor);
        self
    }

    pub fn with_algorithm<S: Into<String>>(
        mut self,
        algorithm_name: S,
        algorithm: Box<
            dyn Algorithm<
                ScabbardProcess,
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
        let unprocessed_action_source = self.unprocessed_action_source.ok_or_else(|| {
            InvalidStateError::with_message("A unprocessed_action_source must be provided".into())
        })?;

        let unprocessed_event_source = self.unprocessed_event_source.ok_or_else(|| {
            InvalidStateError::with_message("A unprocessed_event_source must be provided".into())
        })?;

        let context_source = self.context_source.ok_or_else(|| {
            InvalidStateError::with_message("A context_source must be provided".into())
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
            unprocessed_action_source,
            action_runner: ConsensusActionRunner::new(
                message_sender_factory,
                Box::new(ScabbardStoreContextUpdater::new(
                    scabbard_store_factory.clone(),
                )),
                notify_observer,
                scabbard_store_factory,
            ),
            unprocessed_event_source,
            context_source,
            algorithms: self.algorithms,
            consensus_store_command_factory,
            store_command_executor,
        })
    }
}
