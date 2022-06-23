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

use diesel::prelude::*;
use splinter::service::FullyQualifiedServiceId;

use crate::store::scabbard_store::diesel::{
    models::{Consensus2pcContextModel, Consensus2pcContextParticipantModel},
    schema::{consensus_2pc_context, consensus_2pc_context_participant},
};
use crate::store::scabbard_store::{ConsensusContext, ScabbardStoreError};

use super::ScabbardStoreOperations;

const OPERATION_NAME: &str = "get_current_consensus_context";

pub(in crate::store::scabbard_store::diesel) trait GetCurrentContextAction {
    fn get_current_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<ConsensusContext>, ScabbardStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> GetCurrentContextAction for ScabbardStoreOperations<'a, SqliteConnection> {
    fn get_current_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<ConsensusContext>, ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            let context = consensus_2pc_context::table
                .filter(
                    consensus_2pc_context::circuit_id
                        .eq(service_id.circuit_id().to_string())
                        .and(
                            consensus_2pc_context::service_id
                                .eq(service_id.service_id().to_string()),
                        ),
                )
                .first::<Consensus2pcContextModel>(self.conn)
                .optional()
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            if let Some(context) = context {
                let participants: Vec<Consensus2pcContextParticipantModel> =
                    consensus_2pc_context_participant::table
                        .filter(
                            consensus_2pc_context_participant::circuit_id
                                .eq(service_id.circuit_id().to_string())
                                .and(
                                    consensus_2pc_context_participant::service_id
                                        .eq(service_id.service_id().to_string()),
                                ),
                        )
                        .load::<Consensus2pcContextParticipantModel>(self.conn)
                        .map_err(|err| {
                            ScabbardStoreError::from_source_with_operation(
                                err,
                                OPERATION_NAME.to_string(),
                            )
                        })?;

                Ok(Some(ConsensusContext::try_from((&context, participants))?))
            } else {
                Ok(None)
            }
        })
    }
}

#[cfg(feature = "postgres")]
impl<'a> GetCurrentContextAction for ScabbardStoreOperations<'a, PgConnection> {
    fn get_current_consensus_context(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<ConsensusContext>, ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            let context = consensus_2pc_context::table
                .filter(
                    consensus_2pc_context::circuit_id
                        .eq(service_id.circuit_id().to_string())
                        .and(
                            consensus_2pc_context::service_id
                                .eq(service_id.service_id().to_string()),
                        ),
                )
                .first::<Consensus2pcContextModel>(self.conn)
                .optional()
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            if let Some(context) = context {
                let participants: Vec<Consensus2pcContextParticipantModel> =
                    consensus_2pc_context_participant::table
                        .filter(
                            consensus_2pc_context_participant::circuit_id
                                .eq(service_id.circuit_id().to_string())
                                .and(
                                    consensus_2pc_context_participant::service_id
                                        .eq(service_id.service_id().to_string()),
                                ),
                        )
                        .load::<Consensus2pcContextParticipantModel>(self.conn)
                        .map_err(|err| {
                            ScabbardStoreError::from_source_with_operation(
                                err,
                                OPERATION_NAME.to_string(),
                            )
                        })?;

                Ok(Some(ConsensusContext::try_from((&context, participants))?))
            } else {
                Ok(None)
            }
        })
    }
}
