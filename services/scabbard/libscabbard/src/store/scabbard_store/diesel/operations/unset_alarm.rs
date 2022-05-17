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

#[cfg(feature = "postgres")]
use diesel::pg::PgConnection;
#[cfg(feature = "sqlite")]
use diesel::sqlite::SqliteConnection;
use diesel::{delete, prelude::*};
use splinter::error::InvalidStateError;
use splinter::service::FullyQualifiedServiceId;

use crate::store::scabbard_store::diesel::{
    models::{ScabbardAlarmModel, ScabbardServiceModel},
    schema::{scabbard_alarm, scabbard_service},
};
use crate::store::scabbard_store::ScabbardStoreError;
use crate::store::AlarmType;

use super::ScabbardStoreOperations;

pub(in crate::store::scabbard_store::diesel) trait UnsetAlarmOperation {
    fn unset_alarm(
        &self,
        service_id: &FullyQualifiedServiceId,
        alarm_type: &AlarmType,
    ) -> Result<(), ScabbardStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> UnsetAlarmOperation for ScabbardStoreOperations<'a, SqliteConnection> {
    fn unset_alarm(
        &self,
        service_id: &FullyQualifiedServiceId,
        alarm_type: &AlarmType,
    ) -> Result<(), ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            // check to see if a service with the given service_id exists
            scabbard_service::table
                .filter(scabbard_service::service_id.eq(format!("{}", service_id)))
                .first::<ScabbardServiceModel>(self.conn)
                .optional()?
                .ok_or_else(|| {
                    ScabbardStoreError::InvalidState(InvalidStateError::with_message(String::from(
                        "Failed to unset scabbard alarm, service does not exist",
                    )))
                })?;

            let current_alarm = scabbard_alarm::table
                .filter(
                    scabbard_alarm::service_id
                        .eq(format!("{}", service_id))
                        .and(scabbard_alarm::alarm_type.eq(String::from(alarm_type))),
                )
                .first::<ScabbardAlarmModel>(self.conn)
                .optional()?;

            if current_alarm.is_some() {
                // delete the current alarm
                delete(
                    scabbard_alarm::table.filter(
                        scabbard_alarm::service_id
                            .eq(format!("{}", service_id))
                            .and(scabbard_alarm::alarm_type.eq(String::from(alarm_type))),
                    ),
                )
                .execute(self.conn)?;
            }

            Ok(())
        })
    }
}

#[cfg(feature = "postgres")]
impl<'a> UnsetAlarmOperation for ScabbardStoreOperations<'a, PgConnection> {
    fn unset_alarm(
        &self,
        service_id: &FullyQualifiedServiceId,
        alarm_type: &AlarmType,
    ) -> Result<(), ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            // check to see if a service with the given service_id exists
            scabbard_service::table
                .filter(scabbard_service::service_id.eq(format!("{}", service_id)))
                .first::<ScabbardServiceModel>(self.conn)
                .optional()?
                .ok_or_else(|| {
                    ScabbardStoreError::InvalidState(InvalidStateError::with_message(String::from(
                        "Failed to unset scabbard alarm, service does not exist",
                    )))
                })?;

            let current_alarm = scabbard_alarm::table
                .filter(
                    scabbard_alarm::service_id
                        .eq(format!("{}", service_id))
                        .and(scabbard_alarm::alarm_type.eq(String::from(alarm_type))),
                )
                .first::<ScabbardAlarmModel>(self.conn)
                .optional()?;

            if current_alarm.is_some() {
                // delete the current alarm
                delete(
                    scabbard_alarm::table.filter(
                        scabbard_alarm::service_id
                            .eq(format!("{}", service_id))
                            .and(scabbard_alarm::alarm_type.eq(String::from(alarm_type))),
                    ),
                )
                .execute(self.conn)?;
            }

            Ok(())
        })
    }
}
