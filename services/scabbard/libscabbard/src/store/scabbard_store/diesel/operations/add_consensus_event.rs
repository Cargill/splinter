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
use splinter::error::{InternalError, InvalidStateError};
use splinter::service::FullyQualifiedServiceId;

use crate::store::scabbard_store::diesel::{
    models::{
        Consensus2pcContextModel, Consensus2pcDeliverEventModel, Consensus2pcStartEventModel,
        Consensus2pcVoteEventModel, InsertableConsensus2pcEventModel,
    },
    schema::{
        consensus_2pc_context, consensus_2pc_deliver_event, consensus_2pc_event,
        consensus_2pc_start_event, consensus_2pc_vote_event,
    },
};
use crate::store::scabbard_store::ScabbardStoreError;
use crate::store::scabbard_store::{
    two_phase_commit::{Event, Message},
    ConsensusEvent,
};

use super::ScabbardStoreOperations;

const OPERATION_NAME: &str = "add_consensus_event";

pub(in crate::store::scabbard_store::diesel) trait AddEventOperation {
    fn add_consensus_event(
        &self,
        service_id: &FullyQualifiedServiceId,
        event: ConsensusEvent,
    ) -> Result<i64, ScabbardStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> AddEventOperation for ScabbardStoreOperations<'a, SqliteConnection> {
    fn add_consensus_event(
        &self,
        service_id: &FullyQualifiedServiceId,
        event: ConsensusEvent,
    ) -> Result<i64, ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            let ConsensusEvent::TwoPhaseCommit(event) = event;
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
                        "Cannot add consensus event, context with service ID {} does not exist",
                        service_id,
                    )))
                })?;

            let position = consensus_2pc_event::table
                .filter(consensus_2pc_event::service_id.eq(format!("{}", service_id)))
                .order(consensus_2pc_event::position.desc())
                .select(consensus_2pc_event::position)
                .first::<i32>(self.conn)
                .optional()
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?
                .unwrap_or(0)
                + 1;

            let insertable_event = InsertableConsensus2pcEventModel {
                service_id: format!("{}", service_id),
                executed_at: None,
                position,
                event_type: String::from(&event),
            };

            insert_into(consensus_2pc_event::table)
                .values(vec![insertable_event])
                .execute(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;
            let event_id = consensus_2pc_event::table
                .order(consensus_2pc_event::id.desc())
                .select(consensus_2pc_event::id)
                .first::<i64>(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            match event {
                Event::Alarm() => Ok(event_id),
                Event::Deliver(receiving_process, message) => {
                    let (message_type, vote_response, vote_request, epoch) = match message {
                        Message::DecisionRequest(epoch) => {
                            (String::from(&message), None, None, epoch)
                        }
                        Message::VoteResponse(epoch, true) => (
                            String::from(&message),
                            Some("TRUE".to_string()),
                            None,
                            epoch,
                        ),
                        Message::VoteResponse(epoch, false) => (
                            String::from(&message),
                            Some("FALSE".to_string()),
                            None,
                            epoch,
                        ),
                        Message::Commit(epoch) => (String::from(&message), None, None, epoch),
                        Message::Abort(epoch) => (String::from(&message), None, None, epoch),
                        Message::VoteRequest(epoch, ref value) => {
                            (String::from(&message), None, Some(value.clone()), epoch)
                        }
                    };

                    let deliver_event = Consensus2pcDeliverEventModel {
                        event_id,
                        epoch: i64::try_from(epoch)
                            .map_err(|err| InternalError::from_source(Box::new(err)))?,
                        receiver_service_id: format!("{}", receiving_process),
                        message_type,
                        vote_response,
                        vote_request,
                    };
                    insert_into(consensus_2pc_deliver_event::table)
                        .values(vec![deliver_event])
                        .execute(self.conn)
                        .map_err(|err| {
                            ScabbardStoreError::from_source_with_operation(
                                err,
                                OPERATION_NAME.to_string(),
                            )
                        })?;
                    Ok(event_id)
                }
                Event::Start(value) => {
                    let start_event = Consensus2pcStartEventModel { event_id, value };
                    insert_into(consensus_2pc_start_event::table)
                        .values(vec![start_event])
                        .execute(self.conn)
                        .map_err(|err| {
                            ScabbardStoreError::from_source_with_operation(
                                err,
                                OPERATION_NAME.to_string(),
                            )
                        })?;
                    Ok(event_id)
                }
                Event::Vote(vote) => {
                    let vote = match vote {
                        true => String::from("TRUE"),
                        false => String::from("FALSE"),
                    };
                    let vote_event = Consensus2pcVoteEventModel { event_id, vote };
                    insert_into(consensus_2pc_vote_event::table)
                        .values(vec![vote_event])
                        .execute(self.conn)
                        .map_err(|err| {
                            ScabbardStoreError::from_source_with_operation(
                                err,
                                OPERATION_NAME.to_string(),
                            )
                        })?;
                    Ok(event_id)
                }
            }
        })
    }
}

#[cfg(feature = "postgres")]
impl<'a> AddEventOperation for ScabbardStoreOperations<'a, PgConnection> {
    fn add_consensus_event(
        &self,
        service_id: &FullyQualifiedServiceId,
        event: ConsensusEvent,
    ) -> Result<i64, ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            let ConsensusEvent::TwoPhaseCommit(event) = event;
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
                        "Cannot add consensus event, context with service ID {} does not exist",
                        service_id,
                    )))
                })?;

            let position = consensus_2pc_event::table
                .filter(consensus_2pc_event::service_id.eq(format!("{}", service_id)))
                .order(consensus_2pc_event::position.desc())
                .select(consensus_2pc_event::position)
                .first::<i32>(self.conn)
                .optional()
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?
                .unwrap_or(0)
                + 1;

            let insertable_event = InsertableConsensus2pcEventModel {
                service_id: format!("{}", service_id),
                executed_at: None,
                position,
                event_type: String::from(&event),
            };

            let event_id: i64 = insert_into(consensus_2pc_event::table)
                .values(vec![insertable_event])
                .returning(consensus_2pc_event::id)
                .get_result(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            match event {
                Event::Alarm() => Ok(event_id),
                Event::Deliver(receiving_process, message) => {
                    let (message_type, vote_response, vote_request, epoch) = match message {
                        Message::DecisionRequest(epoch) => {
                            (String::from(&message), None, None, epoch)
                        }
                        Message::VoteResponse(epoch, true) => (
                            String::from(&message),
                            Some("TRUE".to_string()),
                            None,
                            epoch,
                        ),
                        Message::VoteResponse(epoch, false) => (
                            String::from(&message),
                            Some("FALSE".to_string()),
                            None,
                            epoch,
                        ),
                        Message::Commit(epoch) => (String::from(&message), None, None, epoch),
                        Message::Abort(epoch) => (String::from(&message), None, None, epoch),
                        Message::VoteRequest(epoch, ref value) => {
                            (String::from(&message), None, Some(value.clone()), epoch)
                        }
                    };

                    let deliver_event = Consensus2pcDeliverEventModel {
                        event_id,
                        epoch: i64::try_from(epoch)
                            .map_err(|err| InternalError::from_source(Box::new(err)))?,
                        receiver_service_id: format!("{}", receiving_process),
                        message_type,
                        vote_response,
                        vote_request,
                    };
                    insert_into(consensus_2pc_deliver_event::table)
                        .values(vec![deliver_event])
                        .execute(self.conn)
                        .map_err(|err| {
                            ScabbardStoreError::from_source_with_operation(
                                err,
                                OPERATION_NAME.to_string(),
                            )
                        })?;
                    Ok(event_id)
                }
                Event::Start(value) => {
                    let start_event = Consensus2pcStartEventModel { event_id, value };
                    insert_into(consensus_2pc_start_event::table)
                        .values(vec![start_event])
                        .execute(self.conn)
                        .map_err(|err| {
                            ScabbardStoreError::from_source_with_operation(
                                err,
                                OPERATION_NAME.to_string(),
                            )
                        })?;
                    Ok(event_id)
                }
                Event::Vote(vote) => {
                    let vote = match vote {
                        true => String::from("TRUE"),
                        false => String::from("FALSE"),
                    };
                    let vote_event = Consensus2pcVoteEventModel { event_id, vote };
                    insert_into(consensus_2pc_vote_event::table)
                        .values(vec![vote_event])
                        .execute(self.conn)
                        .map_err(|err| {
                            ScabbardStoreError::from_source_with_operation(
                                err,
                                OPERATION_NAME.to_string(),
                            )
                        })?;
                    Ok(event_id)
                }
            }
        })
    }
}
