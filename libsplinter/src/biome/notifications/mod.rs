/*
 * Copyright 2019 Cargill Incorporated
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 * -----------------------------------------------------------------------------
 */
pub mod database;
mod error;
pub mod notification_manager;
pub(crate) mod rest_api_resources;

use super::error::ModelConversionError;
use database::models::{NotificationModel, NotificationPropertyModel, UserNotificationModel};
pub use error::NotificationManagerError;
use std::collections::HashMap;
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct BiomeNewNotification {
    payload_title: String,
    payload_body: String,
    recipients: Vec<String>,
    properties: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BiomeDbNotification {
    notification_model: NotificationModel,
    user_notifications: Vec<UserNotificationModel>,
    notification_properties: Vec<NotificationPropertyModel>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BiomeNotification {
    notification_id: String,
    title: String,
    body: String,
    created: SystemTime,
    user_id: String,
    properties: HashMap<String, String>,
}

pub trait NotificationManager<T> {
    /// Adds a notification to the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `notification` - The notification to be added
    ///
    ///
    fn add_notification(&self, notification: T) -> Result<(), NotificationManagerError>;
}
impl BiomeNotification {
    // fn from(
    //     notification: NotificationModel,
    //     user_notification: UserNotificationModel,
    //     notification_properties: Vec<NotificationPropertyModel>
    // ) -> BiomeNotification {
    //         BiomeNotification {
    //             notification_id: notification.id,
    //             title: notification.payload_title,
    //             body: notification.payload_body,
    //             created: notification.created,
    //             user_id: user_notification.user_id,
    //             properties: HashMap::new()
    //     }
    // }
}

impl BiomeNewNotification {
    fn into_db_models(self) -> Result<BiomeDbNotification, ModelConversionError> {
        let id = Uuid::new_v4().to_string();
        let created = SystemTime::now();
        Ok(BiomeDbNotification {
            notification_model: NotificationModel::from(&self, &id, created)?,
            user_notifications: UserNotificationModel::from_all(&self, &id)?,
            notification_properties: NotificationPropertyModel::from_all(&self, &id)?,
        })
    }
}

impl NotificationModel {
    fn from(
        notification: &BiomeNewNotification,
        id: &str,
        created: SystemTime,
    ) -> Result<NotificationModel, ModelConversionError> {
        Ok(NotificationModel {
            id: id.to_string(),
            payload_title: notification.payload_title.to_string(),
            payload_body: notification.payload_body.to_string(),
            created: created,
            recipients: notification.recipients.to_vec(),
        })
    }
}

impl UserNotificationModel {
    fn from_all(
        notification: &BiomeNewNotification,
        id: &str,
    ) -> Result<Vec<UserNotificationModel>, ModelConversionError> {
        let mut user_notifications = Vec::new();
        for user in notification.recipients.to_vec() {
            user_notifications.push(UserNotificationModel {
                notification_id: id.to_string(),
                user_id: user,
                unread: true,
            });
        }
        Ok(user_notifications)
    }
}

impl NotificationPropertyModel {
    fn from_all(
        notification: &BiomeNewNotification,
        id: &str,
    ) -> Result<Vec<NotificationPropertyModel>, ModelConversionError> {
        let mut notification_properties = Vec::new();
        for (key, value) in notification.properties.iter() {
            notification_properties.push(NotificationPropertyModel {
                notification_id: id.to_string(),
                property: key.to_string(),
                property_value: value.to_string(),
            });
        }
        Ok(notification_properties)
    }
}
