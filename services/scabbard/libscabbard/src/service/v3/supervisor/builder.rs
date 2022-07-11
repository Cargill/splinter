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

//! Contains the builder for the `Supervisor`

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::SystemTime;

use splinter::error::{InternalError, InvalidStateError};
use splinter::service::{FullyQualifiedServiceId, ServiceType, TimerAlarmFactory};
use splinter::store::command::{StoreCommand, StoreCommandExecutor};

use crate::store::{
    CommitEntryBuilder, ConsensusDecision, ConsensusEvent, Event, Identified,
    PooledScabbardStoreFactory, ScabbardStore, ScabbardStoreFactory, SupervisorNotification,
    SupervisorNotificationType,
};

use super::commands::{
    AddCommitEntryCommand, AddEventCommand, ExecuteSupervisorCommand, UpdateCommitEntryCommand,
};
use super::{Supervisor, SupervisorMessage};

const SCABBARD_SERVICE_TYPE: ServiceType<'static> = ServiceType::new_static("scabbard:v3");

/// Used to build the `Supervisor`
pub struct SupervisorBuilder<E>
where
    E: StoreCommandExecutor + 'static,
    <E as StoreCommandExecutor>::Context: Sized,
{
    pooled_scabbard_store_factory: Option<Arc<dyn PooledScabbardStoreFactory>>,
    scabbard_store_factory:
        Option<Arc<dyn ScabbardStoreFactory<<E as StoreCommandExecutor>::Context>>>,
    store_command_executor: Option<Arc<E>>,
    notifier_channel: Option<(Sender<SupervisorMessage>, Receiver<SupervisorMessage>)>,
    timer_alarm_factory: Option<Box<dyn TimerAlarmFactory>>,
}

impl<E> Default for SupervisorBuilder<E>
where
    E: StoreCommandExecutor + 'static,
    <E as StoreCommandExecutor>::Context: Sized,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<E> SupervisorBuilder<E>
where
    E: StoreCommandExecutor + 'static,
    <E as StoreCommandExecutor>::Context: Sized,
{
    pub fn new() -> Self {
        SupervisorBuilder {
            pooled_scabbard_store_factory: None,
            scabbard_store_factory: None,
            store_command_executor: None,
            notifier_channel: None,
            timer_alarm_factory: None,
        }
    }

    pub fn with_pooled_scabbard_store_factory(
        mut self,
        scabbard_store: Arc<dyn PooledScabbardStoreFactory>,
    ) -> Self {
        self.pooled_scabbard_store_factory = Some(scabbard_store);
        self
    }

    pub fn with_scabbard_store_factory(
        mut self,
        scabbard_store_factory: Arc<dyn ScabbardStoreFactory<<E as StoreCommandExecutor>::Context>>,
    ) -> Self {
        self.scabbard_store_factory = Some(scabbard_store_factory);
        self
    }

    pub fn with_store_command_executor(mut self, store_command_executor: Arc<E>) -> Self {
        self.store_command_executor = Some(store_command_executor);
        self
    }

    pub fn with_notifier_channel(
        mut self,
        sender: Sender<SupervisorMessage>,
        recv: Receiver<SupervisorMessage>,
    ) -> Self {
        self.notifier_channel = Some((sender, recv));
        self
    }

    pub fn with_timer_alarm_factory(
        mut self,
        timer_alarm_factory: Box<dyn TimerAlarmFactory>,
    ) -> Self {
        self.timer_alarm_factory = Some(timer_alarm_factory);
        self
    }

    pub fn build(self) -> Result<Supervisor, InvalidStateError> {
        let pooled_scabbard_store_factory =
            self.pooled_scabbard_store_factory.ok_or_else(|| {
                InvalidStateError::with_message("A 'scabbard_store' must be provided".into())
            })?;

        let scabbard_store_factory = self.scabbard_store_factory.ok_or_else(|| {
            InvalidStateError::with_message("A 'scabbard_store_factory' must be provided".into())
        })?;

        let store_command_executor = self.store_command_executor.ok_or_else(|| {
            InvalidStateError::with_message("A 'store_command_executor' must be provided".into())
        })?;

        let timer_alarm_factory = self.timer_alarm_factory.ok_or_else(|| {
            InvalidStateError::with_message("A 'timer_alarm' must be provided".into())
        })?;

        // if a the sender and receiver have not already been provided, create a channel
        let (sender, notification_recv) = if let Some((sender, recv)) = self.notifier_channel {
            (sender, recv)
        } else {
            channel()
        };

        let join_handle =
            thread::Builder::new()
                .name("ScabbardSupervisor".into())
                .spawn(move || {
                    let scabbard_store = pooled_scabbard_store_factory.new_store();
                    loop {
                        match notification_recv.recv() {
                            Ok(SupervisorMessage::Notification(service_id)) => {
                                let notifications = match scabbard_store
                                    .list_supervisor_notifications(&service_id)
                                {
                                    Ok(notifications) => notifications,
                                    Err(err) => {
                                        error!(
                                            "Unable to fetch pending supervisor notifications :{}",
                                            err
                                        );
                                        continue;
                                    }
                                };

                                let mut wake_up = false;
                                for notification in notifications {
                                    if matches!(
                                    notification.record.notification_type(),
                                    SupervisorNotificationType::RequestForStart
                                        | SupervisorNotificationType::CoordinatorRequestForVote
                                        | SupervisorNotificationType::ParticipantRequestForVote{..}
                                ) {
                                        wake_up = true
                                    }
                                    let commands = match handle_notification::<
                                        <E as StoreCommandExecutor>::Context,
                                    >(
                                        &service_id,
                                        notification,
                                        &scabbard_store_factory,
                                        &scabbard_store,
                                    ) {
                                        Ok(commands) => commands,
                                        Err(err) => {
                                            error!(
                                                "Unable to handle supervisor notification: {}",
                                                err
                                            );
                                            // break iteration of for loop. should not keep handling
                                            // notifications
                                            break;
                                        }
                                    };

                                    if let Err(err) = store_command_executor.execute(commands) {
                                        error!(
                                            "Unable to execute the commands to handle supervisor \
                                        notification: {}",
                                            err
                                        );
                                        // break  iteration of for loop. should not keep handling
                                        // notifications
                                        break;
                                    }
                                }

                                if wake_up {
                                    if let Err(err) = timer_alarm_factory
                                        .new_alarm()
                                        .wake_up(SCABBARD_SERVICE_TYPE, Some(service_id.clone()))
                                    {
                                        error!("Unable to send wake_up to Timer: {}", err)
                                    }
                                }
                            }
                            Ok(SupervisorMessage::Shutdown) => {
                                debug!("Supervisor received shutdown");
                                break;
                            }
                            Err(err) => {
                                error!("Supervisor unable to receive message: {}", err);
                                break;
                            }
                        }
                    }
                })
                .map_err(|_| {
                    InvalidStateError::with_message("Unable to start Supervisor thread".into())
                })?;

        Ok(Supervisor {
            sender,
            join_handle,
        })
    }
}

fn handle_notification<C: 'static>(
    service_id: &FullyQualifiedServiceId,
    notification: Identified<SupervisorNotification>,
    scabbard_store_factory: &Arc<dyn ScabbardStoreFactory<C>>,
    scabbard_store: &dyn ScabbardStore,
) -> Result<Vec<Box<dyn StoreCommand<Context = C>>>, InternalError> {
    let mut commands: Vec<Box<dyn StoreCommand<Context = C>>> = Vec::new();

    match notification.record.notification_type() {
        SupervisorNotificationType::Commit => {
            if let Some(commit_entry) = scabbard_store
                .get_last_commit_entry(service_id)
                .map_err(|err| InternalError::from_source(Box::new(err)))?
            {
                let updated_commit_entry = commit_entry
                    .into_builder()
                    .with_decision(&ConsensusDecision::Commit)
                    .build()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;

                commands.push(Box::new(UpdateCommitEntryCommand::new(
                    scabbard_store_factory.clone(),
                    updated_commit_entry,
                )))
            } else {
                return Err(InternalError::with_message(
                    "Received commit for unknown entry".to_string(),
                ));
            }
        }
        SupervisorNotificationType::Abort => {
            if let Some(commit_entry) = scabbard_store
                .get_last_commit_entry(service_id)
                .map_err(|err| InternalError::from_source(Box::new(err)))?
            {
                let updated_commit_entry = commit_entry
                    .into_builder()
                    .with_decision(&ConsensusDecision::Abort)
                    .build()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;

                commands.push(Box::new(UpdateCommitEntryCommand::new(
                    scabbard_store_factory.clone(),
                    updated_commit_entry,
                )));
            } else {
                return Err(InternalError::with_message(
                    "Received abort for unknown entry".to_string(),
                ));
            }
        }
        SupervisorNotificationType::RequestForStart => {
            // Use the current system time as a string for the value that will be agreed upon
            let s = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_err(|err| InternalError::from_source(Box::new(err)))?
                .as_secs()
                .to_string();

            let entry = CommitEntryBuilder::default()
                .with_service_id(service_id)
                .with_value(&s)
                .build()
                .map_err(|err| InternalError::from_source(Box::new(err)))?;

            commands.push(Box::new(AddEventCommand::new(
                scabbard_store_factory.clone(),
                service_id.clone(),
                ConsensusEvent::TwoPhaseCommit(Event::Start(s.into_bytes())),
            )));
            commands.push(Box::new(AddCommitEntryCommand::new(
                scabbard_store_factory.clone(),
                entry,
            )));
        }
        SupervisorNotificationType::CoordinatorRequestForVote => {
            commands.push(Box::new(AddEventCommand::new(
                scabbard_store_factory.clone(),
                service_id.clone(),
                ConsensusEvent::TwoPhaseCommit(Event::Vote(true)),
            )));
        }
        SupervisorNotificationType::ParticipantRequestForVote { value } => {
            let entry = CommitEntryBuilder::default()
                .with_service_id(service_id)
                .with_value(
                    &String::from_utf8(value.to_vec())
                        .map_err(|err| InternalError::from_source(Box::new(err)))?,
                )
                .build()
                .map_err(|err| InternalError::from_source(Box::new(err)))?;

            commands.push(Box::new(AddEventCommand::new(
                scabbard_store_factory.clone(),
                service_id.clone(),
                ConsensusEvent::TwoPhaseCommit(Event::Vote(true)),
            )));
            commands.push(Box::new(AddCommitEntryCommand::new(
                scabbard_store_factory.clone(),
                entry,
            )));
        }
    };

    commands.push(Box::new(ExecuteSupervisorCommand::new(
        scabbard_store_factory.clone(),
        service_id.clone(),
        notification.id,
    )));

    Ok(commands)
}
