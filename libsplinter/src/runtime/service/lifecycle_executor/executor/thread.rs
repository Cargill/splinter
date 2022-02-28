// Copyright 2018-2022 Cargill Incorporated
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you mcay not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! The internal thread used by the LifecycleExecutor

use std::marker::PhantomData;
use std::sync::mpsc::Receiver;
use std::thread;

use crate::error::InternalError;
use crate::runtime::service::{
    LifecycleCommand, LifecycleCommandGenerator, LifecycleService, LifecycleStatus, LifecycleStore,
};
use crate::service::{FullyQualifiedServiceId, ServiceType};
use crate::store::command::StoreCommandExecutor;

use super::{message::ExecutorMessage, LifecycleMap};

pub struct ExecutorThread<E: 'static>
where
    E: StoreCommandExecutor + Send,
{
    join_handle: thread::JoinHandle<()>,
    _executor: PhantomData<E>,
}

impl<E> ExecutorThread<E>
where
    E: StoreCommandExecutor + Send,
{
    pub fn start(
        recv: Receiver<ExecutorMessage>,
        lifecycles: LifecycleMap<E::Context>,
        store: Box<dyn LifecycleStore + Send>,
        command_generator: LifecycleCommandGenerator<E::Context>,
        command_executor: E,
    ) -> Result<ExecutorThread<E>, InternalError> {
        let join_handle = thread::Builder::new()
            .name("LifecycleExecutorMainThread".to_string())
            .spawn(move || loop {
                match recv.recv() {
                    Ok(ExecutorMessage::WakeUpAll) => {
                        wake_up_all(&lifecycles, &*store, &command_generator, &command_executor)
                    }
                    Ok(ExecutorMessage::WakeUp {
                        service_type,
                        service_id,
                    }) => wake_up(
                        &lifecycles,
                        &*store,
                        &command_generator,
                        &command_executor,
                        service_type,
                        service_id,
                    ),
                    Ok(ExecutorMessage::Shutdown) => {
                        debug!("LifecycleExecutor received shutdown");
                        break;
                    }
                    Err(_) => {
                        error!("LifecycleExecutor timer channel dropped");
                        break;
                    }
                }
            })
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        Ok(ExecutorThread {
            join_handle,
            _executor: PhantomData,
        })
    }

    pub fn join(self) -> Result<(), InternalError> {
        self.join_handle.join().map_err(|err| {
            InternalError::with_message(format!(
                "LifecycleExecutor thread did not shutdown correctly: {:?}",
                err
            ))
        })
    }
}

fn wake_up_all<E>(
    lifecycles: &LifecycleMap<E::Context>,
    store: &dyn LifecycleStore,
    command_generator: &LifecycleCommandGenerator<E::Context>,
    command_executor: &E,
) where
    E: StoreCommandExecutor + Send,
{
    let services = match store.list_services(&LifecycleStatus::New) {
        Ok(services) => services,
        Err(err) => {
            error!("Unable able to check for pending services: {}", err);
            return;
        }
    };

    handle_command(lifecycles, services, command_generator, command_executor);
}

fn wake_up<E>(
    lifecycles: &LifecycleMap<E::Context>,
    store: &dyn LifecycleStore,
    command_generator: &LifecycleCommandGenerator<E::Context>,
    command_executor: &E,
    service_type: ServiceType<'static>,
    service_id: Option<FullyQualifiedServiceId>,
) where
    E: StoreCommandExecutor + Send,
{
    let services = match store.list_services(&LifecycleStatus::New) {
        Ok(services) => {
            let mut services: Vec<LifecycleService> = services
                .into_iter()
                .filter(|service| service.service_type() == &service_type)
                .collect();

            if let Some(service_id) = &service_id {
                services = match services
                    .into_iter()
                    .find(|service| service.service_id() == service_id)
                {
                    Some(service) => vec![service],
                    None => {
                        error!(
                            "No pending work found for service {} (service type: {})",
                            service_type, service_id,
                        );
                        return;
                    }
                }
            };

            services
        }
        Err(err) => {
            error!("Unable able to check for pending services: {}", err);
            return;
        }
    };

    handle_command(lifecycles, services, command_generator, command_executor);
}

fn handle_command<E>(
    lifecycles: &LifecycleMap<E::Context>,
    services: Vec<LifecycleService>,
    command_generator: &LifecycleCommandGenerator<E::Context>,
    command_executor: &E,
) where
    E: StoreCommandExecutor + Send,
{
    for service in services {
        debug!(
            "Handling service {} (service type: {}) with command {}",
            service.service_id(),
            service.service_type(),
            service.command(),
        );
        let lifecycle = match lifecycles.get(service.service_type()) {
            Some(lifecycle) => lifecycle,
            None => {
                error!(
                    "No lifecycle found for service {} (service type: {})",
                    service.service_id(),
                    service.service_type()
                );
                continue;
            }
        };
        let mut service_commands = {
            let command_result = match service.command() {
                LifecycleCommand::Prepare => lifecycle
                    .command_to_prepare(service.service_id().clone(), service.arguments().to_vec()),
                LifecycleCommand::Finalize => {
                    lifecycle.command_to_finalize(service.service_id().clone())
                }
                LifecycleCommand::Retire => {
                    lifecycle.command_to_retire(service.service_id().clone())
                }
                LifecycleCommand::Purge => lifecycle.command_to_purge(service.service_id().clone()),
            };

            match command_result {
                Ok(commands) => vec![commands],
                Err(err) => {
                    error!(
                        "Unable to get lifecycle commands for service {} (service type: {}): {}",
                        service.service_id(),
                        service.service_type(),
                        err
                    );
                    continue;
                }
            }
        };

        match command_generator.complete_service(service.clone()) {
            Ok(new_command) => service_commands.push(new_command),
            Err(err) => {
                error!(
                    "Unable to get lifecycle commands for service {} (service type: {}): {}",
                    service.service_id(),
                    service.service_type(),
                    err
                );
                continue;
            }
        }

        if let Err(err) = command_executor.execute(service_commands) {
            error!(
                "Unable to execute lifecycle commands for service {} (service type: {}): {}",
                service.service_id(),
                service.service_type(),
                err
            );
            continue;
        }
    }
}
