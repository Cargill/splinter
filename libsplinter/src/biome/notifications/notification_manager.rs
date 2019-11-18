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
use super::database::helpers::{
    insert_notification, insert_notification_property, insert_user_notification,
};
use super::{BiomeNewNotification, NotificationManager, NotificationManagerError};
use crate::database::ConnectionPool;

pub struct BiomeNotificationManager {
    connection_pool: ConnectionPool,
}

impl NotificationManager<BiomeNewNotification> for BiomeNotificationManager {
    /// Adds a notifications to the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `notification` - The notification to be added
    ///
    ///
    fn add_notification(
        &self,
        notification: BiomeNewNotification,
    ) -> Result<(), NotificationManagerError> {
        let db_models = notification.into_db_models()?;
        insert_notification(&*self.connection_pool.get()?, db_models.notification_model).map_err(
            |err| {
                NotificationManagerError::OperationError(format!(
                    "Failed to add notification: {}",
                    err
                ))
            },
        )?;
        for user_notification in db_models.user_notifications {
            insert_user_notification(&*self.connection_pool.get()?, user_notification).map_err(
                |err| {
                    NotificationManagerError::OperationError(format!(
                        "Failed to add user_notification: {}",
                        err
                    ))
                },
            )?;
        }
        for property in db_models.notification_properties {
            insert_notification_property(&*self.connection_pool.get()?, property).map_err(
                |err| {
                    NotificationManagerError::OperationError(format!(
                        "Failed to add notification property: {}",
                        err
                    ))
                },
            )?;
        }
        Ok(())
    }
}
