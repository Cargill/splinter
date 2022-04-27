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
use diesel::{dsl::insert_into, prelude::*};
use splinter::error::InternalError;
use splinter::service::FullyQualifiedServiceId;

use crate::store::scabbard_store::diesel::{
    models::{
        Consensus2pcCoordinatorContextModel, Consensus2pcParticipantContextModel,
        CoordinatorContextParticipantList, ParticipantContextParticipantList,
    },
    schema::{
        consensus_2pc_consensus_coordinator_context,
        consensus_2pc_consensus_coordinator_context_participant, consensus_2pc_participant_context,
        consensus_2pc_participant_context_participant,
    },
};
use crate::store::scabbard_store::ScabbardContext;
use crate::store::scabbard_store::ScabbardStoreError;

use super::ScabbardStoreOperations;

pub(in crate::store::scabbard_store::diesel) trait AddContextOperation {
    fn add_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ScabbardContext,
    ) -> Result<(), ScabbardStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> AddContextOperation for ScabbardStoreOperations<'a, SqliteConnection> {
    fn add_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ScabbardContext,
    ) -> Result<(), ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            match context {
                ScabbardContext::Scabbard2pcContext(context) => {
                    match Consensus2pcCoordinatorContextModel::try_from((&context, service_id)) {
                        Ok(new_coordinator_context) => {
                            let participants = CoordinatorContextParticipantList::try_from((
                                &context, service_id,
                            ))?
                            .inner;
                            insert_into(consensus_2pc_consensus_coordinator_context::table)
                                .values(vec![new_coordinator_context])
                                .execute(self.conn)?;
                            insert_into(
                                consensus_2pc_consensus_coordinator_context_participant::table,
                            )
                            .values(participants)
                            .execute(self.conn)?;
                        }
                        Err(_) => match Consensus2pcParticipantContextModel::try_from((
                            &context, service_id,
                        )) {
                            Ok(new_participant_context) => {
                                let participants = ParticipantContextParticipantList::try_from((
                                    &context, service_id,
                                ))?
                                .inner;
                                insert_into(consensus_2pc_participant_context::table)
                                    .values(vec![new_participant_context])
                                    .execute(self.conn)?;
                                insert_into(consensus_2pc_participant_context_participant::table)
                                    .values(participants)
                                    .execute(self.conn)?;
                            }
                            Err(e) => {
                                return Err(ScabbardStoreError::Internal(
                                    InternalError::from_source(Box::new(e)),
                                ))
                            }
                        },
                    }
                }
            }
            Ok(())
        })
    }
}

#[cfg(feature = "postgres")]
impl<'a> AddContextOperation for ScabbardStoreOperations<'a, PgConnection> {
    fn add_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
        context: ScabbardContext,
    ) -> Result<(), ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            match context {
                ScabbardContext::Scabbard2pcContext(context) => {
                    match Consensus2pcCoordinatorContextModel::try_from((&context, service_id)) {
                        Ok(new_coordinator_context) => {
                            let participants = CoordinatorContextParticipantList::try_from((
                                &context, service_id,
                            ))?
                            .inner;
                            insert_into(consensus_2pc_consensus_coordinator_context::table)
                                .values(vec![new_coordinator_context])
                                .execute(self.conn)?;
                            insert_into(
                                consensus_2pc_consensus_coordinator_context_participant::table,
                            )
                            .values(participants)
                            .execute(self.conn)?;
                        }
                        Err(_) => match Consensus2pcParticipantContextModel::try_from((
                            &context, service_id,
                        )) {
                            Ok(new_participant_context) => {
                                let participants = ParticipantContextParticipantList::try_from((
                                    &context, service_id,
                                ))?
                                .inner;
                                insert_into(consensus_2pc_participant_context::table)
                                    .values(vec![new_participant_context])
                                    .execute(self.conn)?;
                                insert_into(consensus_2pc_participant_context_participant::table)
                                    .values(participants)
                                    .execute(self.conn)?;
                            }
                            Err(e) => {
                                return Err(ScabbardStoreError::Internal(
                                    InternalError::from_source(Box::new(e)),
                                ))
                            }
                        },
                    }
                }
            }
            Ok(())
        })
    }
}
