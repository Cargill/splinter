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

use chrono::naive::NaiveDateTime;
use diesel::{prelude::*, update};
use splinter::error::InternalError;
use splinter::service::FullyQualifiedServiceId;

use crate::store::scabbard_store::diesel::schema::supervisor_notification;
use crate::store::scabbard_store::ScabbardStoreError;

use super::ScabbardStoreOperations;

const OPERATION_NAME: &str = "update_supervisor_notification";

pub(in crate::store::scabbard_store::diesel) trait UpdateSupervisorNotificationOperation {
    fn update_supervisor_notification(
        &self,
        service_id: &FullyQualifiedServiceId,
        notification: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError>;
}

impl<'a, C> UpdateSupervisorNotificationOperation for ScabbardStoreOperations<'a, C>
where
    C: diesel::Connection,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
    NaiveDateTime: diesel::serialize::ToSql<diesel::sql_types::Timestamp, C::Backend>,
{
    fn update_supervisor_notification(
        &self,
        service_id: &FullyQualifiedServiceId,
        notification_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            let update_executed_at = get_naive_date_time(executed_at)?;

            update(supervisor_notification::table)
                .filter(
                    supervisor_notification::id
                        .eq(notification_id)
                        .and(
                            supervisor_notification::service_id
                                .eq(service_id.service_id().as_str()),
                        )
                        .and(
                            supervisor_notification::circuit_id
                                .eq(service_id.circuit_id().as_str()),
                        ),
                )
                .set(supervisor_notification::executed_at.eq(Some(update_executed_at)))
                .execute(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;
            Ok(())
        })
    }
}

fn get_naive_date_time(time: SystemTime) -> Result<NaiveDateTime, InternalError> {
    let duration = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|err| InternalError::from_source(Box::new(err)))?;
    let seconds = i64::try_from(duration.as_secs())
        .map_err(|err| InternalError::from_source(Box::new(err)))?;
    Ok(NaiveDateTime::from_timestamp(
        seconds,
        duration.subsec_nanos(),
    ))
}
