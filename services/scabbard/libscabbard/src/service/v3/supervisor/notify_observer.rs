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

//! Contains an implementation of a `NotifyObserver` that works with the `Supervisor`

use std::sync::Arc;

use splinter::error::InternalError;
use splinter::service::FullyQualifiedServiceId;
use splinter::store::command::StoreCommand;

use crate::service::v3::NotifyObserver;
use crate::store::{
    Notification, ScabbardStoreFactory, SupervisorNotificationBuilder, SupervisorNotificationType,
};

use super::commands::AddSupervisorNotificationCommand;

/// Implementation of `NotifyObserver` that works with the `Supervisor`
pub struct SupervisorNotifyObserver<C: 'static> {
    store_factory: Arc<dyn ScabbardStoreFactory<C>>,
}

impl<C: 'static> SupervisorNotifyObserver<C> {
    /// Create a new `CommandNotifyObserver`
    ///
    /// # Arguments
    ///
    /// * `store_factory` - The scabbard store factory to be used by commands
    pub fn new(store_factory: Arc<dyn ScabbardStoreFactory<C>>) -> Self {
        Self { store_factory }
    }
}

impl<C: 'static> NotifyObserver<C> for SupervisorNotifyObserver<C> {
    /// Notify components about consensus notification
    ///
    /// # Arguments
    ///
    /// * `notification` - The notification that needs to be handled
    /// * `service_id` - The service ID of of the service the notification is for
    fn notify(
        &self,
        notification: Notification,
        service_id: &FullyQualifiedServiceId,
        action_id: i64,
    ) -> Result<Vec<Box<dyn StoreCommand<Context = C>>>, InternalError> {
        let mut builder = SupervisorNotificationBuilder::default()
            .with_service_id(service_id)
            .with_action_id(action_id);

        match notification {
            // Generates a new value to agree on and creates commands to add a commit entry to
            // track the value and an new event to start agreement on that value
            Notification::RequestForStart() => {
                builder =
                    builder.with_notification_type(&SupervisorNotificationType::RequestForStart)
            }
            // if we are the coordinator always, vote yes
            Notification::CoordinatorRequestForVote() => {
                builder = builder
                    .with_notification_type(&SupervisorNotificationType::CoordinatorRequestForVote)
            }
            // vote on a the provided value
            // creates commands to add a new commit entry and an event for voting
            Notification::ParticipantRequestForVote(value) => {
                builder = builder.with_notification_type(
                    &SupervisorNotificationType::ParticipantRequestForVote { value },
                )
            }
            // Commit pending value
            // creates a command to update the current commit entry with status committed
            Notification::Commit() => {
                builder = builder.with_notification_type(&SupervisorNotificationType::Commit)
            }
            // Abort pending value
            // creates a command to update the current commit entry with status aborted
            Notification::Abort() => {
                builder = builder.with_notification_type(&SupervisorNotificationType::Abort)
            }
            // log dropped message
            Notification::MessageDropped(msg) => {
                trace!("Message Dropper: {}", msg);
                return Ok(vec![]);
            }
        };

        let supervisor_notification = builder
            .build()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        Ok(vec![Box::new(AddSupervisorNotificationCommand::new(
            self.store_factory.clone(),
            supervisor_notification,
        ))])
    }
}
