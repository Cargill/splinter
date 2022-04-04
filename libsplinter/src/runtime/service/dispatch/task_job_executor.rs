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

use crate::error::{InternalError, InvalidStateError};
use crate::service::{
    FullyQualifiedServiceId, MessageHandler, MessageHandlerFactory, MessageSenderFactory,
};
use crate::threading::{
    lifecycle::ShutdownHandle,
    pool::{JobExecutor, ThreadPool, ThreadPoolBuilder},
};

use super::task::MessageHandlerTaskRunner;

/// Builds [`MessageHandlerTaskPool`] instances.
#[derive(Default)]
pub struct MessageHandlerTaskPoolBuilder {
    thread_pool_builder: ThreadPoolBuilder,
    prefix: Option<String>,
}

impl MessageHandlerTaskPoolBuilder {
    /// Construct a new builder.
    pub fn new() -> Self {
        Self {
            thread_pool_builder: ThreadPoolBuilder::new(),
            prefix: None,
        }
    }

    /// Set the prefix for the worker threads in the pool.
    pub fn with_prefix(mut self, prefix: String) -> Self {
        self.prefix = Some(prefix);
        self
    }

    /// Set the size of the pool.
    pub fn with_size(mut self, size: usize) -> Self {
        self.thread_pool_builder = self.thread_pool_builder.with_size(size);
        self
    }

    /// Constructs the task pool.
    ///
    /// # Errors
    ///
    /// Will return an [`InvalidStateError`] if the pool has not be configured with a size.
    pub fn build(self) -> Result<MessageHandlerTaskPool, InvalidStateError> {
        let thread_pool = self
            .thread_pool_builder
            .with_prefix(
                self.prefix
                    .unwrap_or_else(|| "MessageHandlerTaskPool".to_string()),
            )
            .build()
            .map_err(|err| InvalidStateError::with_message(err.to_string()))?;

        Ok(MessageHandlerTaskPool { thread_pool })
    }
}

/// A pool of [`MessageHandlerTaskRunner`] instances.
pub struct MessageHandlerTaskPool {
    thread_pool: ThreadPool,
}

impl MessageHandlerTaskPool {
    /// Returns a [`MessageHandlerTaskRunner`] instance.
    pub fn task_runner(&self) -> impl MessageHandlerTaskRunner + Send {
        JobExecutorMessageHandlerTaskRunner::new(self.thread_pool.executor())
    }
}

impl ShutdownHandle for MessageHandlerTaskPool {
    fn signal_shutdown(&mut self) {
        self.thread_pool.shutdown_signaler().shutdown();
    }

    fn wait_for_shutdown(self) -> Result<(), InternalError> {
        self.thread_pool.join_all();

        Ok(())
    }
}

struct JobExecutorMessageHandlerTaskRunner {
    job_executor: JobExecutor,
}

impl JobExecutorMessageHandlerTaskRunner {
    fn new(job_executor: JobExecutor) -> Self {
        Self { job_executor }
    }
}

impl MessageHandlerTaskRunner for JobExecutorMessageHandlerTaskRunner {
    fn execute(
        &self,
        message_handler_factory: &dyn MessageHandlerFactory<
            MessageHandler = Box<dyn MessageHandler<Message = Vec<u8>>>,
        >,
        sender_factory: &dyn MessageSenderFactory<Vec<u8>>,
        to_service: FullyQualifiedServiceId,
        from_service: FullyQualifiedServiceId,
        message: Vec<u8>,
    ) -> Result<(), InternalError> {
        let factory = message_handler_factory.clone_boxed();
        let sender_factory = sender_factory.clone_boxed();

        self.job_executor.execute(move || {
            let mut handler = factory.new_handler();
            let sender = match sender_factory.new_message_sender(&to_service) {
                Ok(sender) => sender,
                Err(err) => {
                    error!(
                        "Unable to create new message sender while handling message {} -> {}: {}",
                        to_service, from_service, err
                    );
                    return;
                }
            };

            if let Err(err) =
                handler.handle_message(&*sender, to_service.clone(), from_service.clone(), message)
            {
                error!(
                    "Unable to handle service message {} -> {}: {}",
                    to_service, from_service, err
                );
            }
        });

        Ok(())
    }
}
