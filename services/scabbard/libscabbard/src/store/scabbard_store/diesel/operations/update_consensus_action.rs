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

use diesel::{prelude::*, update};
use splinter::error::{InternalError, InvalidStateError};
use splinter::service::FullyQualifiedServiceId;

use crate::store::scabbard_store::diesel::{
    models::{CoordinatorContextModel, ParticipantContextModel},
    schema::{consensus_action, consensus_coordinator_context, consensus_participant_context},
};
use crate::store::scabbard_store::ScabbardStoreError;

use super::ScabbardStoreOperations;

pub(in crate::store::scabbard_store::diesel) trait UpdateActionOperation {
    fn update_consensus_action(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
        action_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError>;
}

impl<'a, C> UpdateActionOperation for ScabbardStoreOperations<'a, C>
where
    C: diesel::Connection,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn update_consensus_action(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
        action_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            let epoch = i64::try_from(epoch).map_err(|err| {
                ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
            })?;
            // check to see if action a context exists with the given service_id and epoch
            let coordinator_context =
                consensus_coordinator_context::table
                    .filter(consensus_coordinator_context::epoch.eq(epoch).and(
                        consensus_coordinator_context::service_id.eq(format!("{}", service_id)),
                    ))
                    .first::<CoordinatorContextModel>(self.conn)
                    .optional()?;

            let participant_context =
                consensus_participant_context::table
                    .filter(consensus_participant_context::epoch.eq(epoch).and(
                        consensus_participant_context::service_id.eq(format!("{}", service_id)),
                    ))
                    .first::<ParticipantContextModel>(self.conn)
                    .optional()?;

            let update_executed_at = i64::try_from(
                executed_at
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map_err(|err| {
                        ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                    })?
                    .as_secs(),
            )
            .map_err(|err| {
                ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
            })?;

            if coordinator_context.is_some() || participant_context.is_some() {
                update(consensus_action::table)
                    .filter(
                        consensus_action::id
                            .eq(action_id)
                            .and(consensus_action::service_id.eq(format!("{}", service_id)))
                            .and(consensus_action::epoch.eq(epoch)),
                    )
                    .set(consensus_action::executed_at.eq(Some(update_executed_at)))
                    .execute(self.conn)
                    .map(|_| ())
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                Ok(())
            } else {
                Err(ScabbardStoreError::InvalidState(
                    InvalidStateError::with_message(format!(
                        "Faild to update 'executed at' time for consensus action, a context with
                        service_id: {} and epoch: {} does not exist",
                        service_id, epoch
                    )),
                ))
            }
        })
    }
}
