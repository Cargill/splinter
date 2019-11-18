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
use super::super::models::{NotificationModel, NotificationPropertyModel, UserNotificationModel};
use super::super::schema::{notification_properties, notifications, user_notifications};

use diesel::{dsl::insert_into, pg::PgConnection, prelude::*, QueryResult};

pub fn insert_notification(
    conn: &PgConnection,
    notification: NotificationModel,
) -> QueryResult<()> {
    insert_into(notifications::table)
        .values(&vec![notification])
        .execute(conn)
        .map(|_| ())
}

pub fn insert_user_notification(
    conn: &PgConnection,
    user_notification: UserNotificationModel,
) -> QueryResult<()> {
    insert_into(user_notifications::table)
        .values(&vec![user_notification])
        .execute(conn)
        .map(|_| ())
}

pub fn insert_notification_property(
    conn: &PgConnection,
    notification_property: NotificationPropertyModel,
) -> QueryResult<()> {
    insert_into(notification_properties::table)
        .values(&vec![notification_property])
        .execute(conn)
        .map(|_| ())
}
