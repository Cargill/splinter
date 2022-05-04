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
use diesel::{delete, dsl::insert_into, prelude::*};
use splinter::error::{InternalError, InvalidStateError};
use splinter::service::FullyQualifiedServiceId;

use crate::store::scabbard_store::diesel::{
    models::{
        Consensus2pcCoordinatorContextModel, Consensus2pcParticipantContextModel,
        CoordinatorContextParticipantList, ParticipantContextParticipantList,
    },
    schema::{
        consensus_2pc_coordinator_context, consensus_2pc_coordinator_context_participant,
        consensus_2pc_participant_context, consensus_2pc_participant_context_participant,
    },
};
use crate::store::scabbard_store::ScabbardContext;
use crate::store::scabbard_store::ScabbardStoreError;

use super::ScabbardStoreOperations;

pub(in crate::store::scabbard_store::diesel) trait UpdateContextAction {
    fn update_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ScabbardContext,
    ) -> Result<(), ScabbardStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> UpdateContextAction for ScabbardStoreOperations<'a, SqliteConnection> {
    fn update_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ScabbardContext,
    ) -> Result<(), ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            match context {
                ScabbardContext::Scabbard2pcContext(context) => {
                    let epoch = i64::try_from(*context.epoch()).map_err(|err| {
                        ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                    })?;
                    // check to make sure the context exists
                    let coordinator_context = consensus_2pc_coordinator_context::table
                        .filter(
                            consensus_2pc_coordinator_context::epoch
                                .eq(epoch)
                                .and(
                                    consensus_2pc_coordinator_context::service_id
                                        .eq(format!("{}", service_id)),
                                )
                                .and(
                                    consensus_2pc_coordinator_context::coordinator
                                        .eq(format!("{}", context.coordinator())),
                                ),
                        )
                        .first::<Consensus2pcCoordinatorContextModel>(self.conn)
                        .optional()?;

                    let participant_context = consensus_2pc_participant_context::table
                        .filter(
                            consensus_2pc_participant_context::epoch
                                .eq(epoch)
                                .and(
                                    consensus_2pc_participant_context::service_id
                                        .eq(format!("{}", service_id)),
                                )
                                .and(
                                    consensus_2pc_participant_context::coordinator
                                        .eq(format!("{}", context.coordinator())),
                                ),
                        )
                        .first::<Consensus2pcParticipantContextModel>(self.conn)
                        .optional()?;

                    if coordinator_context.is_some() {
                        // return an error if there is both a coordinator and a participant context
                        // for the given service_id and epoch
                        if participant_context.is_some() {
                            return Err(ScabbardStoreError::InvalidState(
                                InvalidStateError::with_message(format!(
                                    "Failed to update consensus action, contexts found for
                                        participant and coordinator with service_id: {} epoch: {} ",
                                    service_id, epoch
                                )),
                            ));
                        }

                        let update_coordinator_context =
                            Consensus2pcCoordinatorContextModel::try_from((&context, service_id))?;

                        delete(
                            consensus_2pc_coordinator_context::table.filter(
                                consensus_2pc_coordinator_context::epoch
                                    .eq(epoch)
                                    .and(
                                        consensus_2pc_coordinator_context::service_id
                                            .eq(format!("{}", service_id)),
                                    )
                                    .and(
                                        consensus_2pc_coordinator_context::coordinator
                                            .eq(format!("{}", context.coordinator())),
                                    ),
                            ),
                        )
                        .execute(self.conn)?;

                        insert_into(consensus_2pc_coordinator_context::table)
                            .values(vec![update_coordinator_context])
                            .execute(self.conn)?;

                        let updated_participants =
                            CoordinatorContextParticipantList::try_from((&context, service_id))?
                                .inner;

                        delete(
                            consensus_2pc_coordinator_context_participant::table.filter(
                                consensus_2pc_coordinator_context_participant::service_id
                                    .eq(format!("{}", service_id))
                                    .and(
                                        consensus_2pc_coordinator_context_participant::epoch
                                            .eq(epoch),
                                    ),
                            ),
                        )
                        .execute(self.conn)?;

                        insert_into(consensus_2pc_coordinator_context_participant::table)
                            .values(updated_participants)
                            .execute(self.conn)?;

                        Ok(())
                    } else if participant_context.is_some() {
                        let update_participant_context =
                            Consensus2pcParticipantContextModel::try_from((&context, service_id))?;

                        delete(
                            consensus_2pc_participant_context::table.filter(
                                consensus_2pc_participant_context::epoch
                                    .eq(epoch)
                                    .and(
                                        consensus_2pc_participant_context::service_id
                                            .eq(format!("{}", service_id)),
                                    )
                                    .and(
                                        consensus_2pc_participant_context::coordinator
                                            .eq(format!("{}", context.coordinator())),
                                    ),
                            ),
                        )
                        .execute(self.conn)?;

                        insert_into(consensus_2pc_participant_context::table)
                            .values(vec![update_participant_context])
                            .execute(self.conn)?;

                        let updated_participants =
                            ParticipantContextParticipantList::try_from((&context, service_id))?
                                .inner;

                        delete(
                            consensus_2pc_participant_context_participant::table.filter(
                                consensus_2pc_participant_context_participant::service_id
                                    .eq(format!("{}", service_id))
                                    .and(
                                        consensus_2pc_participant_context_participant::epoch
                                            .eq(epoch),
                                    ),
                            ),
                        )
                        .execute(self.conn)?;

                        insert_into(consensus_2pc_participant_context_participant::table)
                            .values(updated_participants)
                            .execute(self.conn)?;

                        Ok(())
                    } else {
                        Err(ScabbardStoreError::InvalidState(
                            InvalidStateError::with_message(format!(
                                "Faild to update context, a context with service_id: {} and \
                                epoch: {} does not exist",
                                service_id, epoch
                            )),
                        ))
                    }
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
        context: ScabbardContext,
    ) -> Result<(), ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            match context {
                ScabbardContext::Scabbard2pcContext(context) => {
                    let epoch = i64::try_from(*context.epoch()).map_err(|err| {
                        ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                    })?;
                    // check to make sure the context exists
                    let coordinator_context = consensus_2pc_coordinator_context::table
                        .filter(
                            consensus_2pc_coordinator_context::epoch
                                .eq(epoch)
                                .and(
                                    consensus_2pc_coordinator_context::service_id
                                        .eq(format!("{}", service_id)),
                                )
                                .and(
                                    consensus_2pc_coordinator_context::coordinator
                                        .eq(format!("{}", context.coordinator())),
                                ),
                        )
                        .first::<Consensus2pcCoordinatorContextModel>(self.conn)
                        .optional()?;

                    let participant_context = consensus_2pc_participant_context::table
                        .filter(
                            consensus_2pc_participant_context::epoch.eq(epoch).and(
                                consensus_2pc_participant_context::service_id
                                    .eq(format!("{}", service_id)),
                            ),
                        )
                        .first::<Consensus2pcParticipantContextModel>(self.conn)
                        .optional()?;

                    if coordinator_context.is_some() {
                        // return an error if there is both a coordinator and a participant context
                        // for the given service_id and epoch
                        if participant_context.is_some() {
                            return Err(ScabbardStoreError::InvalidState(
                                InvalidStateError::with_message(format!(
                                    "Failed to update consensus action, contexts found for
                                        participant and coordinator with service_id: {} epoch: {} ",
                                    service_id, epoch
                                )),
                            ));
                        }

                        let update_coordinator_context =
                            Consensus2pcCoordinatorContextModel::try_from((&context, service_id))?;

                        delete(
                            consensus_2pc_coordinator_context::table.filter(
                                consensus_2pc_coordinator_context::epoch
                                    .eq(epoch)
                                    .and(
                                        consensus_2pc_coordinator_context::service_id
                                            .eq(format!("{}", service_id)),
                                    )
                                    .and(
                                        consensus_2pc_coordinator_context::coordinator
                                            .eq(format!("{}", context.coordinator())),
                                    ),
                            ),
                        )
                        .execute(self.conn)?;

                        insert_into(consensus_2pc_coordinator_context::table)
                            .values(vec![update_coordinator_context])
                            .execute(self.conn)?;

                        let updated_participants =
                            CoordinatorContextParticipantList::try_from((&context, service_id))?
                                .inner;

                        delete(
                            consensus_2pc_coordinator_context_participant::table.filter(
                                consensus_2pc_coordinator_context_participant::service_id
                                    .eq(format!("{}", service_id))
                                    .and(
                                        consensus_2pc_coordinator_context_participant::epoch
                                            .eq(epoch),
                                    ),
                            ),
                        )
                        .execute(self.conn)?;

                        insert_into(consensus_2pc_coordinator_context_participant::table)
                            .values(updated_participants)
                            .execute(self.conn)?;

                        Ok(())
                    } else if participant_context.is_some() {
                        let update_participant_context =
                            Consensus2pcParticipantContextModel::try_from((&context, service_id))?;

                        delete(
                            consensus_2pc_participant_context::table.filter(
                                consensus_2pc_participant_context::epoch
                                    .eq(epoch)
                                    .and(
                                        consensus_2pc_participant_context::service_id
                                            .eq(format!("{}", service_id)),
                                    )
                                    .and(
                                        consensus_2pc_participant_context::coordinator
                                            .eq(format!("{}", context.coordinator())),
                                    ),
                            ),
                        )
                        .execute(self.conn)?;

                        insert_into(consensus_2pc_participant_context::table)
                            .values(vec![update_participant_context])
                            .execute(self.conn)?;

                        let updated_participants =
                            ParticipantContextParticipantList::try_from((&context, service_id))?
                                .inner;

                        delete(
                            consensus_2pc_participant_context_participant::table.filter(
                                consensus_2pc_participant_context_participant::service_id
                                    .eq(format!("{}", service_id))
                                    .and(
                                        consensus_2pc_participant_context_participant::epoch
                                            .eq(epoch),
                                    ),
                            ),
                        )
                        .execute(self.conn)?;

                        insert_into(consensus_2pc_participant_context_participant::table)
                            .values(updated_participants)
                            .execute(self.conn)?;

                        Ok(())
                    } else {
                        Err(ScabbardStoreError::InvalidState(
                            InvalidStateError::with_message(format!(
                                "Faild to update context, a context with service_id: {} and \
                                epoch: {} does not exist",
                                service_id, epoch
                            )),
                        ))
                    }
                }
            }
        })
    }
}
