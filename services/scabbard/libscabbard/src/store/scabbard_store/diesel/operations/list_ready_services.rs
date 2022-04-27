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
    consensus_2pc_action, consensus_2pc_consensus_coordinator_context,
    consensus_2pc_participant_context, scabbard_service,
};
use crate::store::scabbard_store::ScabbardStoreError;

use super::ScabbardStoreOperations;

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
                .load::<String>(self.conn)?
                .into_iter()
                .collect();

            let current_time = get_timestamp(Some(SystemTime::now()))?;

            // get the service IDs of coordinators for which the alarm has passed
            let mut ready_services = consensus_2pc_consensus_coordinator_context::table
                .filter(
                    consensus_2pc_consensus_coordinator_context::service_id
                        .eq_any(&finalized_services)
                        .and(consensus_2pc_consensus_coordinator_context::alarm.le(current_time)),
                )
                .select(consensus_2pc_consensus_coordinator_context::service_id)
                .load::<String>(self.conn)?
                .into_iter()
                .collect::<Vec<String>>();

            // get the service IDs of participants for which the alarm has passed
            ready_services.append(
                &mut consensus_2pc_participant_context::table
                    .filter(
                        consensus_2pc_participant_context::service_id
                            .eq_any(&finalized_services)
                            .and(consensus_2pc_participant_context::alarm.le(current_time)),
                    )
                    .select(consensus_2pc_participant_context::service_id)
                    .load::<String>(self.conn)?
                    .into_iter()
                    .collect::<Vec<String>>(),
            );

            // get the service IDs of any finalized services that have unexecuted actions
            ready_services.append(
                &mut consensus_2pc_action::table
                    .filter(consensus_2pc_action::service_id.eq_any(&finalized_services))
                    .filter(consensus_2pc_action::executed_at.is_null())
                    .select(consensus_2pc_action::service_id)
                    .load::<String>(self.conn)?
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

fn get_timestamp(time: Option<SystemTime>) -> Result<Option<i64>, ScabbardStoreError> {
    match time {
        Some(time) => Ok(Some(
            i64::try_from(
                time.duration_since(SystemTime::UNIX_EPOCH)
                    .map_err(|err| {
                        ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                    })?
                    .as_secs(),
            )
            .map_err(|err| {
                ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
            })?,
        )),
        None => Ok(None),
    }
}
