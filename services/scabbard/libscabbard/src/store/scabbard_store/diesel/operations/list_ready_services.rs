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

use diesel::prelude::*;
use splinter::error::InternalError;
use splinter::service::FullyQualifiedServiceId;

use crate::store::scabbard_store::diesel::schema::{
    consensus_2pc_action, consensus_2pc_event, scabbard_alarm, scabbard_service,
};
use crate::store::scabbard_store::ScabbardStoreError;
use crate::store::AlarmType;

use super::ScabbardStoreOperations;

const OPERATION_NAME: &str = "list_ready_services";

pub(in crate::store::scabbard_store::diesel) trait ListReadyServicesOperation {
    fn list_ready_services(&self) -> Result<Vec<FullyQualifiedServiceId>, ScabbardStoreError>;
}

impl<'a, C> ListReadyServicesOperation for ScabbardStoreOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn list_ready_services(&self) -> Result<Vec<FullyQualifiedServiceId>, ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            // get all services in the finalized state that have peers
            let finalized_services: Vec<String> = scabbard_service::table
                .filter(scabbard_service::status.eq("FINALIZED"))
                .select(scabbard_service::service_id)
                .load::<String>(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?
                .into_iter()
                .collect();

            let current_time = get_timestamp(SystemTime::now())?;

            // get the service IDs of services with alarms which have passed
            let mut ready_services = scabbard_alarm::table
                .filter(
                    scabbard_alarm::service_id
                        .eq_any(&finalized_services)
                        .and(
                            scabbard_alarm::alarm_type.eq(String::from(&AlarmType::TwoPhaseCommit)),
                        )
                        .and(scabbard_alarm::alarm.le(current_time)),
                )
                .select(scabbard_alarm::service_id)
                .load::<String>(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?
                .into_iter()
                .collect::<Vec<String>>();

            // get the service IDs of any finalized services that have unexecuted actions
            ready_services.append(
                &mut consensus_2pc_action::table
                    .filter(consensus_2pc_action::service_id.eq_any(&finalized_services))
                    .filter(consensus_2pc_action::executed_at.is_null())
                    .select(consensus_2pc_action::service_id)
                    .load::<String>(self.conn)
                    .map_err(|err| {
                        ScabbardStoreError::from_source_with_operation(
                            err,
                            OPERATION_NAME.to_string(),
                        )
                    })?
                    .into_iter()
                    .collect::<Vec<String>>(),
            );

            // get the service IDs of any finalized services that have unexecuted events
            ready_services.append(
                &mut consensus_2pc_event::table
                    .filter(consensus_2pc_event::service_id.eq_any(&finalized_services))
                    .filter(consensus_2pc_event::executed_at.is_null())
                    .select(consensus_2pc_event::service_id)
                    .load::<String>(self.conn)
                    .map_err(|err| {
                        ScabbardStoreError::from_source_with_operation(
                            err,
                            OPERATION_NAME.to_string(),
                        )
                    })?
                    .into_iter()
                    .collect::<Vec<String>>(),
            );

            ready_services.sort();
            ready_services.dedup();

            let all_ready_services = ready_services
                .into_iter()
                .map(FullyQualifiedServiceId::new_from_string)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| {
                    ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                })?;

            Ok(all_ready_services)
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
