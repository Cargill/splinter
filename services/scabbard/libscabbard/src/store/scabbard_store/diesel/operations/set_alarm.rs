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
use std::time::SystemTime;

#[cfg(feature = "postgres")]
use diesel::pg::PgConnection;
#[cfg(feature = "sqlite")]
use diesel::sqlite::SqliteConnection;
use diesel::{delete, dsl::insert_into, prelude::*};
use splinter::error::{InternalError, InvalidStateError};
use splinter::service::FullyQualifiedServiceId;

use crate::store::scabbard_store::diesel::{
    models::{AlarmTypeModel, ScabbardAlarmModel, ScabbardServiceModel},
    schema::{scabbard_alarm, scabbard_service},
};
use crate::store::scabbard_store::ScabbardStoreError;
use crate::store::AlarmType;

use super::ScabbardStoreOperations;

const OPERATION_NAME: &str = "set_alarm";

pub(in crate::store::scabbard_store::diesel) trait SetAlarmOperation {
    fn set_alarm(
        &self,
        service_id: &FullyQualifiedServiceId,
        alarm_type: &AlarmType,
        alarm: SystemTime,
    ) -> Result<(), ScabbardStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> SetAlarmOperation for ScabbardStoreOperations<'a, SqliteConnection> {
    fn set_alarm(
        &self,
        service_id: &FullyQualifiedServiceId,
        alarm_type: &AlarmType,
        alarm: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            // check to see if a service with the given service_id exists
            scabbard_service::table
                .filter(
                    scabbard_service::circuit_id
                        .eq(service_id.circuit_id().to_string())
                        .and(scabbard_service::service_id.eq(service_id.service_id().to_string())),
                )
                .first::<ScabbardServiceModel>(self.conn)
                .optional()
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?
                .ok_or_else(|| {
                    ScabbardStoreError::InvalidState(InvalidStateError::with_message(String::from(
                        "Failed to set scabbard alarm, service does not exist",
                    )))
                })?;

            let current_alarm = scabbard_alarm::table
                .filter(
                    scabbard_alarm::circuit_id
                        .eq(service_id.circuit_id().to_string())
                        .and(scabbard_alarm::service_id.eq(service_id.service_id().to_string()))
                        .and(scabbard_alarm::alarm_type.eq(AlarmTypeModel::from(alarm_type))),
                )
                .first::<ScabbardAlarmModel>(self.conn)
                .optional()
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            let new_alarm = ScabbardAlarmModel {
                circuit_id: service_id.circuit_id().to_string(),
                service_id: service_id.service_id().to_string(),
                alarm_type: AlarmTypeModel::from(alarm_type),
                alarm: get_timestamp(alarm)?,
            };

            if current_alarm.is_some() {
                // delete the current alarm
                delete(
                    scabbard_alarm::table.filter(
                        scabbard_alarm::circuit_id
                            .eq(service_id.circuit_id().to_string())
                            .and(scabbard_alarm::service_id.eq(service_id.service_id().to_string()))
                            .and(scabbard_alarm::alarm_type.eq(AlarmTypeModel::from(alarm_type))),
                    ),
                )
                .execute(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;
            }

            insert_into(scabbard_alarm::table)
                .values(vec![new_alarm])
                .execute(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            Ok(())
        })
    }
}

#[cfg(feature = "postgres")]
impl<'a> SetAlarmOperation for ScabbardStoreOperations<'a, PgConnection> {
    fn set_alarm(
        &self,
        service_id: &FullyQualifiedServiceId,
        alarm_type: &AlarmType,
        alarm: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            // check to see if a service with the given service_id exists
            scabbard_service::table
                .filter(
                    scabbard_service::circuit_id
                        .eq(service_id.circuit_id().to_string())
                        .and(scabbard_service::service_id.eq(service_id.service_id().to_string())),
                )
                .first::<ScabbardServiceModel>(self.conn)
                .optional()
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?
                .ok_or_else(|| {
                    ScabbardStoreError::InvalidState(InvalidStateError::with_message(String::from(
                        "Failed to set scabbard alarm, service does not exist",
                    )))
                })?;

            let current_alarm = scabbard_alarm::table
                .filter(
                    scabbard_alarm::circuit_id
                        .eq(service_id.circuit_id().to_string())
                        .and(scabbard_alarm::service_id.eq(service_id.service_id().to_string()))
                        .and(scabbard_alarm::alarm_type.eq(AlarmTypeModel::from(alarm_type))),
                )
                .first::<ScabbardAlarmModel>(self.conn)
                .optional()
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            let new_alarm = ScabbardAlarmModel {
                circuit_id: service_id.circuit_id().to_string(),
                service_id: service_id.service_id().to_string(),
                alarm_type: AlarmTypeModel::from(alarm_type),
                alarm: get_timestamp(alarm)?,
            };

            if current_alarm.is_some() {
                // delete the current alarm
                delete(
                    scabbard_alarm::table.filter(
                        scabbard_alarm::circuit_id
                            .eq(service_id.circuit_id().to_string())
                            .and(scabbard_alarm::service_id.eq(service_id.service_id().to_string()))
                            .and(scabbard_alarm::alarm_type.eq(AlarmTypeModel::from(alarm_type))),
                    ),
                )
                .execute(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;
            }

            insert_into(scabbard_alarm::table)
                .values(vec![new_alarm])
                .execute(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            Ok(())
        })
    }
}

fn get_timestamp(time: SystemTime) -> Result<i64, ScabbardStoreError> {
    i64::try_from(
        time.duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|err| ScabbardStoreError::Internal(InternalError::from_source(Box::new(err))))?
            .as_secs(),
    )
    .map_err(|err| ScabbardStoreError::Internal(InternalError::from_source(Box::new(err))))
}
