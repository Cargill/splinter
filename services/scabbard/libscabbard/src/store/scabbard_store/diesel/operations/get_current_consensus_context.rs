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

use std::cmp::Ordering;
use std::convert::TryFrom;

use diesel::prelude::*;
use splinter::error::{InternalError, InvalidStateError};
use splinter::service::FullyQualifiedServiceId;

use crate::store::scabbard_store::diesel::{
    models::{
        Consensus2pcCoordinatorContextModel, Consensus2pcCoordinatorContextParticipantModel,
        Consensus2pcParticipantContextModel, Consensus2pcParticipantContextParticipantModel,
    },
    schema::{
        consensus_2pc_consensus_coordinator_context,
        consensus_2pc_consensus_coordinator_context_participant, consensus_2pc_participant_context,
        consensus_2pc_participant_context_participant,
    },
};
use crate::store::scabbard_store::{context::ScabbardContext, ScabbardStoreError};

use super::ScabbardStoreOperations;

pub(in crate::store::scabbard_store::diesel) trait GetCurrentContextAction {
    fn get_current_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<ScabbardContext>, ScabbardStoreError>;
}

impl<'a, C> GetCurrentContextAction for ScabbardStoreOperations<'a, C>
where
    C: diesel::Connection,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn get_current_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<ScabbardContext>, ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            let coordinator_context = consensus_2pc_consensus_coordinator_context::table
                .filter(consensus_2pc_consensus_coordinator_context::service_id.eq(format!("{}", service_id)))
                .order(consensus_2pc_consensus_coordinator_context::epoch.desc())
                .first::<Consensus2pcCoordinatorContextModel>(self.conn)
                .optional()?;

            let participant_context = consensus_2pc_participant_context::table
                .filter(consensus_2pc_participant_context::service_id.eq(format!("{}", service_id)))
                .order(consensus_2pc_participant_context::epoch.desc())
                .first::<Consensus2pcParticipantContextModel>(self.conn)
                .optional()?;

            match (coordinator_context, participant_context) {
                // If both a coordinator and a participant contexts exist for the given service ID
                // get the context with the largest epoch
                (Some(coordinator_context), Some(participant_context)) => {
                    match coordinator_context.epoch.cmp(&participant_context.epoch) {
                        // If both a coordinator and a participant context exists for the given
                        // service ID and they have the same epoch value, return an error
                        Ordering::Equal => {
                            return Err(ScabbardStoreError::InvalidState(
                                InvalidStateError::with_message(format!(
                                    "Failed to add consensus action, contexts found for both
                                        participant and coordinator with service_id: {} epoch: {} ",
                                    service_id, participant_context.epoch
                                )),
                            ));
                        }
                        Ordering::Greater => {
                            let coordinator_participants: Vec<
                                Consensus2pcCoordinatorContextParticipantModel,
                            > = consensus_2pc_consensus_coordinator_context_participant::table
                                .filter(
                                    consensus_2pc_consensus_coordinator_context_participant::service_id
                                        .eq(format!("{}", service_id))
                                        .and(
                                            consensus_2pc_consensus_coordinator_context_participant::epoch
                                                .eq(coordinator_context.epoch),
                                        ),
                                )
                                .load::<Consensus2pcCoordinatorContextParticipantModel>(
                                    self.conn,
                                )?;
                            Ok(Some(
                                ScabbardContext::try_from((
                                    &coordinator_context,
                                    coordinator_participants,
                                ))
                                .map_err(|err| {
                                    ScabbardStoreError::Internal(InternalError::from_source(
                                        Box::new(err),
                                    ))
                                })?,
                            ))
                        }
                        Ordering::Less => {
                            let participant_participants: Vec<
                                Consensus2pcParticipantContextParticipantModel,
                            > = consensus_2pc_participant_context_participant::table
                                .filter(
                                    consensus_2pc_participant_context_participant::service_id
                                        .eq(format!("{}", service_id))
                                        .and(
                                            consensus_2pc_participant_context_participant::epoch
                                                .eq(participant_context.epoch),
                                        ),
                                )
                                .load::<Consensus2pcParticipantContextParticipantModel>(
                                    self.conn,
                                )?;
                            Ok(Some(
                                ScabbardContext::try_from((
                                    &participant_context,
                                    participant_participants,
                                ))
                                .map_err(|err| {
                                    ScabbardStoreError::Internal(InternalError::from_source(
                                        Box::new(err),
                                    ))
                                })?,
                            ))
                        }
                    }
                }
                (Some(coordinator_context), None) => {
                    let coordinator_participants: Vec<
                        Consensus2pcCoordinatorContextParticipantModel,
                    > = consensus_2pc_consensus_coordinator_context_participant::table
                        .filter(
                            consensus_2pc_consensus_coordinator_context_participant::service_id
                                .eq(format!("{}", service_id))
                                .and(
                                    consensus_2pc_consensus_coordinator_context_participant::epoch
                                        .eq(coordinator_context.epoch),
                                ),
                        )
                        .load::<Consensus2pcCoordinatorContextParticipantModel>(self.conn)?;
                    Ok(Some(
                        ScabbardContext::try_from((&coordinator_context, coordinator_participants))
                            .map_err(|err| {
                                ScabbardStoreError::Internal(InternalError::from_source(Box::new(
                                    err,
                                )))
                            })?,
                    ))
                }
                (None, Some(participant_context)) => {
                    let participant_participants: Vec<
                        Consensus2pcParticipantContextParticipantModel,
                    > = consensus_2pc_participant_context_participant::table
                        .filter(
                            consensus_2pc_participant_context_participant::service_id
                                .eq(format!("{}", service_id))
                                .and(
                                    consensus_2pc_participant_context_participant::epoch
                                        .eq(participant_context.epoch),
                                ),
                        )
                        .load::<Consensus2pcParticipantContextParticipantModel>(self.conn)?;
                    Ok(Some(
                        ScabbardContext::try_from((&participant_context, participant_participants))
                            .map_err(|err| {
                                ScabbardStoreError::Internal(InternalError::from_source(Box::new(
                                    err,
                                )))
                            })?,
                    ))
                }
                (None, None) => Ok(None),
            }
        })
    }
}
