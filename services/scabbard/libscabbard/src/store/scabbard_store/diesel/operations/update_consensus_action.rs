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
    models::Consensus2pcContextModel,
    schema::{consensus_2pc_action, consensus_2pc_context},
};
use crate::store::scabbard_store::ScabbardStoreError;

use super::ScabbardStoreOperations;

const OPERATION_NAME: &str = "update_consensus_action";

pub(in crate::store::scabbard_store::diesel) trait UpdateActionOperation {
    fn update_consensus_action(
        &self,
        service_id: &FullyQualifiedServiceId,
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
        action_id: i64,
        executed_at: SystemTime,
    ) -> Result<(), ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            // check to see if a context with the given service_id exists
            consensus_2pc_context::table
                .filter(consensus_2pc_context::service_id.eq(format!("{}", service_id)))
                .first::<Consensus2pcContextModel>(self.conn)
                .optional()
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?
                .ok_or_else(|| {
                    ScabbardStoreError::InvalidState(InvalidStateError::with_message(format!(
                        "Context with service ID {} does not exist",
                        service_id,
                    )))
                })?;

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

            update(consensus_2pc_action::table)
                .filter(
                    consensus_2pc_action::id
                        .eq(action_id)
                        .and(consensus_2pc_action::service_id.eq(format!("{}", service_id))),
                )
                .set(consensus_2pc_action::executed_at.eq(Some(update_executed_at)))
                .execute(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;
            Ok(())
        })
    }
}
