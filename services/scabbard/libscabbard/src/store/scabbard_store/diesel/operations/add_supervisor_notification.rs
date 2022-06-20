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

use std::convert::TryFrom;

#[cfg(feature = "postgres")]
use diesel::pg::PgConnection;
#[cfg(feature = "sqlite")]
use diesel::sqlite::SqliteConnection;
use diesel::{dsl::insert_into, prelude::*};

use crate::store::scabbard_store::diesel::{
    models::InsertableSupervisorNotificationModel, schema::supervisor_notification,
};
use crate::store::scabbard_store::ScabbardStoreError;
use crate::store::scabbard_store::SupervisorNotification;

use super::ScabbardStoreOperations;

const OPERATION_NAME: &str = "add_supervisor_notification";

pub(in crate::store::scabbard_store::diesel) trait AddSupervisorNotficationOperation {
    fn add_supervisor_notification(
        &self,
        supervisor_notification: SupervisorNotification,
    ) -> Result<(), ScabbardStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> AddSupervisorNotficationOperation for ScabbardStoreOperations<'a, SqliteConnection> {
    fn add_supervisor_notification(
        &self,
        notification: SupervisorNotification,
    ) -> Result<(), ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            let new_notification = InsertableSupervisorNotificationModel::try_from(&notification)?;

            insert_into(supervisor_notification::table)
                .values(vec![new_notification])
                .execute(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            Ok(())
        })
    }
}

#[cfg(feature = "postgres")]
impl<'a> AddSupervisorNotficationOperation for ScabbardStoreOperations<'a, PgConnection> {
    fn add_supervisor_notification(
        &self,
        notification: SupervisorNotification,
    ) -> Result<(), ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            let new_notification = InsertableSupervisorNotificationModel::try_from(&notification)?;

            insert_into(supervisor_notification::table)
                .values(vec![new_notification])
                .execute(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            Ok(())
        })
    }
}
