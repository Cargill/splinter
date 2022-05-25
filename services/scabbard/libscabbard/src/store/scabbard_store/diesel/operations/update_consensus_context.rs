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
use diesel::{delete, dsl::insert_into, prelude::*, update};
use splinter::error::InvalidStateError;
use splinter::service::FullyQualifiedServiceId;

use crate::store::scabbard_store::diesel::{
    models::{Consensus2pcContextModel, ContextParticipantList},
    schema::{consensus_2pc_context, consensus_2pc_context_participant},
};
use crate::store::scabbard_store::ConsensusContext;
use crate::store::scabbard_store::ScabbardStoreError;

use super::ScabbardStoreOperations;

const OPERATION_NAME: &str = "update_consensus_context";

pub(in crate::store::scabbard_store::diesel) trait UpdateContextAction {
    fn update_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ConsensusContext,
    ) -> Result<(), ScabbardStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> UpdateContextAction for ScabbardStoreOperations<'a, SqliteConnection> {
    fn update_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ConsensusContext,
    ) -> Result<(), ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            match context {
                ConsensusContext::TwoPhaseCommit(context) => {
                    // check to see if a context with the given service_id exists
                    consensus_2pc_context::table
                        .filter(consensus_2pc_context::service_id.eq(format!("{}", service_id)))
                        .first::<Consensus2pcContextModel>(self.conn)
                        .optional()
                        .map_err(|err| {
                            ScabbardStoreError::from_source_with_operation(
                                err,
                                OPERATION_NAME.to_string(),
                            )
                        })?
                        .ok_or_else(|| {
                            ScabbardStoreError::InvalidState(InvalidStateError::with_message(
                                format!("Context with service ID {} does not exist", service_id,),
                            ))
                        })?;

                    let update_context =
                        Consensus2pcContextModel::try_from((&context, service_id))?;

                    update(consensus_2pc_context::table)
                        .filter(consensus_2pc_context::service_id.eq(service_id.to_string()))
                        .set((
                            consensus_2pc_context::coordinator.eq(update_context.coordinator),
                            consensus_2pc_context::epoch.eq(update_context.epoch),
                            consensus_2pc_context::last_commit_epoch
                                .eq(update_context.last_commit_epoch),
                            consensus_2pc_context::state.eq(update_context.state),
                            consensus_2pc_context::vote_timeout_start
                                .eq(update_context.vote_timeout_start),
                            consensus_2pc_context::vote.eq(update_context.vote),
                            consensus_2pc_context::decision_timeout_start
                                .eq(update_context.decision_timeout_start),
                        ))
                        .execute(self.conn)
                        .map_err(|err| {
                            ScabbardStoreError::from_source_with_operation(
                                err,
                                OPERATION_NAME.to_string(),
                            )
                        })?;

                    let updated_participants =
                        ContextParticipantList::try_from((&context, service_id))?.inner;

                    delete(consensus_2pc_context_participant::table.filter(
                        consensus_2pc_context_participant::service_id.eq(format!("{}", service_id)),
                    ))
                    .execute(self.conn)
                    .map_err(|err| {
                        ScabbardStoreError::from_source_with_operation(
                            err,
                            OPERATION_NAME.to_string(),
                        )
                    })?;

                    insert_into(consensus_2pc_context_participant::table)
                        .values(updated_participants)
                        .execute(self.conn)
                        .map_err(|err| {
                            ScabbardStoreError::from_source_with_operation(
                                err,
                                OPERATION_NAME.to_string(),
                            )
                        })?;

                    Ok(())
                }
            }
        })
    }
}

#[cfg(feature = "postgres")]
impl<'a> UpdateContextAction for ScabbardStoreOperations<'a, PgConnection> {
    fn update_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ConsensusContext,
    ) -> Result<(), ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            match context {
                ConsensusContext::TwoPhaseCommit(context) => {
                    // check to see if a context with the given service_id exists
                    consensus_2pc_context::table
                        .filter(consensus_2pc_context::service_id.eq(format!("{}", service_id)))
                        .first::<Consensus2pcContextModel>(self.conn)
                        .optional()
                        .map_err(|err| {
                            ScabbardStoreError::from_source_with_operation(
                                err,
                                OPERATION_NAME.to_string(),
                            )
                        })?
                        .ok_or_else(|| {
                            ScabbardStoreError::InvalidState(InvalidStateError::with_message(
                                format!("Context with service ID {} does not exist", service_id,),
                            ))
                        })?;

                    let update_context =
                        Consensus2pcContextModel::try_from((&context, service_id))?;

                    update(consensus_2pc_context::table)
                        .filter(consensus_2pc_context::service_id.eq(service_id.to_string()))
                        .set((
                            consensus_2pc_context::coordinator.eq(update_context.coordinator),
                            consensus_2pc_context::epoch.eq(update_context.epoch),
                            consensus_2pc_context::last_commit_epoch
                                .eq(update_context.last_commit_epoch),
                            consensus_2pc_context::state.eq(update_context.state),
                            consensus_2pc_context::vote_timeout_start
                                .eq(update_context.vote_timeout_start),
                            consensus_2pc_context::vote.eq(update_context.vote),
                            consensus_2pc_context::decision_timeout_start
                                .eq(update_context.decision_timeout_start),
                        ))
                        .execute(self.conn)
                        .map_err(|err| {
                            ScabbardStoreError::from_source_with_operation(
                                err,
                                OPERATION_NAME.to_string(),
                            )
                        })?;

                    let updated_participants =
                        ContextParticipantList::try_from((&context, service_id))?.inner;

                    delete(consensus_2pc_context_participant::table.filter(
                        consensus_2pc_context_participant::service_id.eq(format!("{}", service_id)),
                    ))
                    .execute(self.conn)
                    .map_err(|err| {
                        ScabbardStoreError::from_source_with_operation(
                            err,
                            OPERATION_NAME.to_string(),
                        )
                    })?;

                    insert_into(consensus_2pc_context_participant::table)
                        .values(updated_participants)
                        .execute(self.conn)
                        .map_err(|err| {
                            ScabbardStoreError::from_source_with_operation(
                                err,
                                OPERATION_NAME.to_string(),
                            )
                        })?;

                    Ok(())
                }
            }
        })
    }
}
