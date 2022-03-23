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

//! The internal thread used by the Timer

use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use crate::error::InternalError;
use crate::service::{FullyQualifiedServiceId, MessageSenderFactory, ServiceType};
use crate::threading::{
    lifecycle::ShutdownHandle,
    pool::{JobExecutor, ThreadPool, ThreadPoolBuilder},
};

use super::message::TimerMessage;
use super::FilterCollection;

const DEFAULT_POOL_SIZE: usize = 8;

pub struct TimerThread {
    join_handle: thread::JoinHandle<()>,
    thread_pool: ThreadPool,
    shutdown_sender: Sender<TimerMessage>,
}

impl TimerThread {
    pub fn start(
        filters: FilterCollection,
        recv: Receiver<TimerMessage>,
        message_sender_factory: Box<dyn MessageSenderFactory<Vec<u8>>>,
        sender: Sender<TimerMessage>,
    ) -> Result<TimerThread, InternalError> {
        let thread_pool = ThreadPoolBuilder::new()
            .with_size(DEFAULT_POOL_SIZE)
            .with_prefix("TimerThread-".into())
            .build()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let executor = thread_pool.executor();

        let join_handle = thread::Builder::new()
            .name("TimerMainThread".to_string())
            .spawn(move || loop {
                match recv.recv() {
                    Ok(TimerMessage::WakeUpAll) => {
                        wake_up_all(&filters, &executor, &*message_sender_factory)
                    }
                    Ok(TimerMessage::WakeUp {
                        service_type,
                        service_id,
                    }) => wake_up(
                        &filters,
                        &executor,
                        &*message_sender_factory,
                        service_type,
                        service_id,
                    ),
                    Ok(TimerMessage::Shutdown) => {
                        debug!("Service timer received shutdown");
                        break;
                    }
                    Err(_) => {
                        error!("Service timer channel dropped");
                        break;
                    }
                }
            })
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        Ok(TimerThread {
            join_handle,
            thread_pool,
            shutdown_sender: sender,
        })
    }
}

impl ShutdownHandle for TimerThread {
    fn signal_shutdown(&mut self) {
        if self.shutdown_sender.send(TimerMessage::Shutdown).is_err() {
            warn!("Timer is no longer running");
        }
        self.thread_pool.shutdown_signaler().shutdown();
    }

    fn wait_for_shutdown(self) -> Result<(), InternalError> {
        debug!("Shutting down timer thread...");
        self.join_handle.join().map_err(|err| {
            InternalError::with_message(format!(
                "Timer thread did not shutdown correctly: {:?}",
                err
            ))
        })?;

        self.thread_pool.join_all();
        debug!("Shutting down timer thread (complete)");
        Ok(())
    }
}

/// Check all filters for pending work
fn wake_up_all(
    filters: &FilterCollection,
    executor: &JobExecutor,
    message_sender_factory: &(dyn MessageSenderFactory<Vec<u8>> + 'static),
) {
    for (filter, handler_factory) in filters {
        let service_ids = match filter.filter() {
            Ok(service_ids) => service_ids,
            Err(err) => {
                warn!("Unable to get service IDs from timer filter: {}", err);
                continue;
            }
        };

        for service_id in service_ids.into_iter() {
            let new_handle_factory = handler_factory.clone_box();
            let msg_sender_factory = message_sender_factory.clone_boxed();
            executor.execute(move || {
                let timer_sender = match msg_sender_factory.new_message_sender(&service_id) {
                    Ok(timer_sender) => timer_sender,
                    Err(err) => {
                        error!("Unable to get message sender: {}", err);
                        return;
                    }
                };

                let mut handler = {
                    match new_handle_factory.new_handler() {
                        Ok(handler) => handler,
                        Err(err) => {
                            error!(
                                "Unable to get timer handler for service \
                                {}: {}",
                                service_id, err
                            );
                            return;
                        }
                    }
                };

                if let Err(err) = handler.handle_timer(&*timer_sender, service_id) {
                    error!("{}", err);
                }
            })
        }
    }
}

/// Check the filter for the provided service type for pending work. If a service id is provided
/// verify it is returned by the filter and only run that handler.
fn wake_up(
    filters: &FilterCollection,
    executor: &JobExecutor,
    message_sender_factory: &(dyn MessageSenderFactory<Vec<u8>> + 'static),
    service_type: ServiceType<'static>,
    service_id: Option<FullyQualifiedServiceId>,
) {
    let (filter, handler_factory) = match filters
        .iter()
        .find(|(filter, _)| filter.service_types().contains(&service_type))
    {
        Some((filter, handler_factory)) => (filter, handler_factory),
        None => {
            error!("No filter for serivce type {}", service_type);
            return;
        }
    };

    let service_ids = match filter.filter() {
        Ok(service_ids) => service_ids,
        Err(err) => {
            warn!("Unable to get service IDs from timer filter: {}", err);
            return;
        }
    };

    if let Some(id) = service_id {
        if service_ids.contains(&id) {
            let new_handle_factory = handler_factory.clone_box();
            let msg_sender_factory = message_sender_factory.clone_boxed();
            executor.execute(move || {
                let timer_sender = match msg_sender_factory.new_message_sender(&id) {
                    Ok(timer_sender) => timer_sender,
                    Err(err) => {
                        error!("Unable to get message sender: {}", err);
                        return;
                    }
                };

                let mut handler = {
                    match new_handle_factory.new_handler() {
                        Ok(handler) => handler,
                        Err(err) => {
                            error!(
                                "Unable to get timer handler for \
                                    service {}: {}",
                                id, err
                            );
                            return;
                        }
                    }
                };

                if let Err(err) = handler.handle_timer(&*timer_sender, id) {
                    error!("{}", err);
                }
            })
        } else {
            warn!(
                "Received a wake up for service id that doesn't have \
                pending work: {}",
                id
            )
        }
    } else {
        for service_id in service_ids.into_iter() {
            let new_handle_factory = handler_factory.clone_box();
            let msg_sender_factory = message_sender_factory.clone_boxed();
            executor.execute(move || {
                let timer_sender = match msg_sender_factory.new_message_sender(&service_id) {
                    Ok(timer_sender) => timer_sender,
                    Err(err) => {
                        error!("Unable to get message sender: {}", err);
                        return;
                    }
                };

                let mut handler = {
                    match new_handle_factory.new_handler() {
                        Ok(handler) => handler,
                        Err(err) => {
                            error!(
                                "Unable to get timer handler for \
                                        service {}: {}",
                                service_id, err
                            );
                            return;
                        }
                    }
                };

                if let Err(err) = handler.handle_timer(&*timer_sender, service_id) {
                    error!("{}", err);
                }
            })
        }
    }
}
