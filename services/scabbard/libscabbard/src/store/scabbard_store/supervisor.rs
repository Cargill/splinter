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

use std::fmt;
use std::time::SystemTime;

use splinter::error::InvalidStateError;
use splinter::service::FullyQualifiedServiceId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupervisorNotification {
    service_id: FullyQualifiedServiceId,
    action_id: i64,
    notification_type: SupervisorNotificationType,
    created_at: Option<SystemTime>,
    executed_at: Option<SystemTime>,
}

impl SupervisorNotification {
    /// Returns the service ID for the notficiation
    pub fn service_id(&self) -> &FullyQualifiedServiceId {
        &self.service_id
    }

    /// Returns the id of the associated action
    pub fn action_id(&self) -> &i64 {
        &self.action_id
    }

    /// Returns the notification_type for the notficiation
    pub fn notification_type(&self) -> &SupervisorNotificationType {
        &self.notification_type
    }

    /// Returns the created_at time for the notficiation
    pub fn created_at(&self) -> &Option<SystemTime> {
        &self.created_at
    }

    /// Returns the executed_at time for the notficiation
    pub fn executed_at(&self) -> &Option<SystemTime> {
        &self.executed_at
    }
}

#[derive(Default)]
pub struct SupervisorNotificationBuilder {
    service_id: Option<FullyQualifiedServiceId>,
    action_id: Option<i64>,
    notification_type: Option<SupervisorNotificationType>,
    created_at: Option<SystemTime>,
    executed_at: Option<SystemTime>,
}

impl SupervisorNotificationBuilder {
    /// Sets the service ID
    ///
    /// # Arguments
    ///
    ///  * `service_id` - The service ID the notification belongs to
    pub fn with_service_id(
        mut self,
        service_id: &FullyQualifiedServiceId,
    ) -> SupervisorNotificationBuilder {
        self.service_id = Some(service_id.clone());
        self
    }

    /// Sets the ID of the associated action
    ///
    /// # Arguments
    ///
    ///  * `action_id` - The ID that represents the associated action
    pub fn with_action_id(mut self, action_id: i64) -> SupervisorNotificationBuilder {
        self.action_id = Some(action_id);
        self
    }

    /// Sets the notification_type
    ///
    /// # Arguments
    ///
    ///  * `notification_type` - The notficiation type of the notification
    pub fn with_notification_type(
        mut self,
        notification_type: &SupervisorNotificationType,
    ) -> SupervisorNotificationBuilder {
        self.notification_type = Some(notification_type.clone());
        self
    }

    /// Sets the executed_at time
    ///
    /// # Arguments
    ///
    ///  * `executed_at` - The timestamp that represents when the notficiation was handled
    pub fn with_executed_at(mut self, executed_at: SystemTime) -> SupervisorNotificationBuilder {
        self.executed_at = Some(executed_at);
        self
    }

    /// Sets the created_at time
    ///
    /// # Arguments
    ///
    ///  * `created_at` - The timestamp that represents when the notficiation was created
    pub fn with_created_at(mut self, created_at: SystemTime) -> SupervisorNotificationBuilder {
        self.created_at = Some(created_at);
        self
    }

    /// Builds the `SupervisorNotification`
    ///
    /// Returns an error if the service ID, action_id, or notification_type is not set
    pub fn build(self) -> Result<SupervisorNotification, InvalidStateError> {
        let service_id = self.service_id.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `service_id`".to_string(),
            )
        })?;

        let action_id = self.action_id.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `action_id`".to_string(),
            )
        })?;

        let notification_type = self.notification_type.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `notification_type`".to_string(),
            )
        })?;

        let created_at = self.created_at;

        let executed_at = self.executed_at;

        Ok(SupervisorNotification {
            service_id,
            action_id,
            notification_type,
            created_at,
            executed_at,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SupervisorNotificationType {
    Abort,
    Commit,
    RequestForStart,
    CoordinatorRequestForVote,
    ParticipantRequestForVote { value: Vec<u8> },
}

impl fmt::Display for SupervisorNotificationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SupervisorNotificationType::Abort => write!(f, "Notification type: Abort"),
            SupervisorNotificationType::Commit => write!(f, "Notification type: Commit"),
            SupervisorNotificationType::RequestForStart => {
                write!(f, "Notification type: RequestForStart")
            }
            SupervisorNotificationType::CoordinatorRequestForVote => {
                write!(f, "Notification type: CoordinatorRequestForVote")
            }
            SupervisorNotificationType::ParticipantRequestForVote { .. } => {
                write!(f, "Notification type: ParticipantRequestForVote")
            }
        }
    }
}
