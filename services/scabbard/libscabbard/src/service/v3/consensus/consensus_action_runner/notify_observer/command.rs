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

//! Contains an implementation of a `NotifyObserver` that operates soley on returning commands

use std::sync::Arc;
use std::time::SystemTime;

use splinter::error::InternalError;
use splinter::service::FullyQualifiedServiceId;
use splinter::store::command::StoreCommand;

use crate::service::v3::consensus::consensus_action_runner::commands::notifications::{
    AddCommitEntryCommand, AddEventCommand, UpdateCommitEntryCommand,
};
use crate::store::Notification;
use crate::store::{CommitEntryBuilder, ConsensusDecision, ConsensusEvent, Event};
use crate::store::{ScabbardStore, ScabbardStoreFactory};

use super::NotifyObserver;

/// Implementation of `NotifyObserver` that operates on `StoreCommands`
pub struct CommandNotifyObserver<C: 'static> {
    store_factory: Arc<dyn ScabbardStoreFactory<C>>,
    store: Box<dyn ScabbardStore>,
}

impl<C: 'static> CommandNotifyObserver<C> {
    /// Create a new `CommandNotifyObserver`
    ///
    /// # Arguments
    ///
    /// * `store_factory` - The scabbard store factory to be used by commands
    /// * `store` - The scabbard store used to fetch current state
    pub fn new(
        store_factory: Arc<dyn ScabbardStoreFactory<C>>,
        store: Box<dyn ScabbardStore>,
    ) -> Self {
        Self {
            store_factory,
            store,
        }
    }
}

impl<C: 'static> NotifyObserver<C> for CommandNotifyObserver<C> {
    /// Notify components about consensus notification
    ///
    /// # Arguments
    ///
    /// * `notification` - The notification that needs to be handled
    /// * `service_id` - The service ID of of the service the notification is for
    /// * `epoch` - The current epoch of the consensus algorithm
    fn notify(
        &self,
        notification: Notification,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<Vec<Box<dyn StoreCommand<Context = C>>>, InternalError> {
        let mut commands: Vec<Box<dyn StoreCommand<Context = C>>> = Vec::new();
        match notification {
            // Generates a new value to agree on and creates commands to add a commit entry to
            // track the value and an new event to start agreement on that value
            Notification::RequestForStart() => {
                // Use the current system time as a string for the value that will be agreed upon
                let s = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map_err(|err| InternalError::from_source(Box::new(err)))?
                    .as_secs()
                    .to_string();

                let entry = CommitEntryBuilder::default()
                    .with_service_id(service_id)
                    .with_value(&s)
                    .with_epoch(epoch)
                    .build()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;

                commands.push(Box::new(AddEventCommand::new(
                    self.store_factory.clone(),
                    service_id.clone(),
                    epoch,
                    ConsensusEvent::TwoPhaseCommit(Event::Start(s.into_bytes())),
                )));
                commands.push(Box::new(AddCommitEntryCommand::new(
                    self.store_factory.clone(),
                    entry,
                )));
            }
            // if we are the coordiantor always, vote yes
            Notification::CoordinatorRequestForVote() => {
                commands.push(Box::new(AddEventCommand::new(
                    self.store_factory.clone(),
                    service_id.clone(),
                    epoch,
                    ConsensusEvent::TwoPhaseCommit(Event::Vote(true)),
                )));
            }
            // vote on a the provided value
            // creates commands to add a new commit entry and an event for voting
            Notification::ParticipantRequestForVote(value) => {
                let entry = CommitEntryBuilder::default()
                    .with_service_id(service_id)
                    .with_value(
                        &String::from_utf8(value)
                            .map_err(|err| InternalError::from_source(Box::new(err)))?,
                    )
                    .with_epoch(epoch)
                    .build()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;

                commands.push(Box::new(AddEventCommand::new(
                    self.store_factory.clone(),
                    service_id.clone(),
                    epoch,
                    ConsensusEvent::TwoPhaseCommit(Event::Vote(true)),
                )));
                commands.push(Box::new(AddCommitEntryCommand::new(
                    self.store_factory.clone(),
                    entry,
                )));
            }
            // Commit pending value
            // creates a command to update the current commit entry withs status commited
            Notification::Commit() => {
                if let Some(commit_entry) = self
                    .store
                    .get_last_commit_entry(service_id)
                    .map_err(|err| InternalError::from_source(Box::new(err)))?
                {
                    let updated_commit_entry = commit_entry
                        .into_builder()
                        .with_decision(&ConsensusDecision::Commit)
                        .build()
                        .map_err(|err| InternalError::from_source(Box::new(err)))?;

                    commands.push(Box::new(UpdateCommitEntryCommand::new(
                        self.store_factory.clone(),
                        updated_commit_entry,
                    )))
                } else {
                    return Err(InternalError::with_message(
                        "Received commit for unknown entry".to_string(),
                    ));
                }
            }
            // Abort pending value
            // creates a command to update the current commit entry withs status aborted
            Notification::Abort() => {
                if let Some(commit_entry) = self
                    .store
                    .get_last_commit_entry(service_id)
                    .map_err(|err| InternalError::from_source(Box::new(err)))?
                {
                    let updated_commit_entry = commit_entry
                        .into_builder()
                        .with_decision(&ConsensusDecision::Abort)
                        .build()
                        .map_err(|err| InternalError::from_source(Box::new(err)))?;

                    commands.push(Box::new(UpdateCommitEntryCommand::new(
                        self.store_factory.clone(),
                        updated_commit_entry,
                    )));
                } else {
                    return Err(InternalError::with_message(format!(
                        "Received abort for unknown entry: epoch {}",
                        epoch
                    )));
                }
            }
            // log dropped message
            Notification::MessageDropped(msg) => {
                trace!("Message Dropper: {}", msg);
                return Ok(vec![]);
            }
        };

        Ok(commands)
    }
}
