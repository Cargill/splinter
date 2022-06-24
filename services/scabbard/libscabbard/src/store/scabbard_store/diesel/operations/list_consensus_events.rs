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

use diesel::prelude::*;
use splinter::error::{InternalError, InvalidStateError};
use splinter::service::FullyQualifiedServiceId;
use splinter::service::ServiceId;

use crate::store::scabbard_store::diesel::{
    models::{
        Consensus2pcDeliverEventModel, Consensus2pcStartEventModel, Consensus2pcVoteEventModel,
        ConsensusTypeModel, ConsensusTypeModelMapping, EventTypeModel, EventTypeModelMapping,
        MessageTypeModel, MessageTypeModelMapping, ScabbardServiceModel, ServiceStatusTypeModel,
        ServiceStatusTypeModelMapping,
    },
    schema::{
        consensus_2pc_deliver_event, consensus_2pc_event, consensus_2pc_start_event,
        consensus_2pc_vote_event, scabbard_service,
    },
};
use crate::store::scabbard_store::ScabbardStoreError;
use crate::store::scabbard_store::{
    event::ConsensusEvent,
    identified::Identified,
    two_phase_commit::{Event, Message},
};

use super::ScabbardStoreOperations;

const OPERATION_NAME: &str = "list_consensus_events";

pub(in crate::store::scabbard_store::diesel) trait ListEventsOperation {
    fn list_consensus_events(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Vec<Identified<ConsensusEvent>>, ScabbardStoreError>;
}

impl<'a, C> ListEventsOperation for ScabbardStoreOperations<'a, C>
where
    C: diesel::Connection,
    i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, C::Backend>,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
    Vec<u8>: diesel::deserialize::FromSql<diesel::sql_types::Binary, C::Backend>,
    <C as diesel::Connection>::Backend: diesel::types::HasSqlType<MessageTypeModelMapping>,
    MessageTypeModel: diesel::deserialize::FromSql<MessageTypeModelMapping, C::Backend>,
    <C as diesel::Connection>::Backend: diesel::types::HasSqlType<ServiceStatusTypeModelMapping>,
    ServiceStatusTypeModel: diesel::deserialize::FromSql<ServiceStatusTypeModelMapping, C::Backend>,
    <C as diesel::Connection>::Backend: diesel::types::HasSqlType<ConsensusTypeModelMapping>,
    ConsensusTypeModel: diesel::deserialize::FromSql<ConsensusTypeModelMapping, C::Backend>,
    <C as diesel::Connection>::Backend: diesel::types::HasSqlType<EventTypeModelMapping>,
    EventTypeModel: diesel::deserialize::FromSql<EventTypeModelMapping, C::Backend>,
{
    fn list_consensus_events(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Vec<Identified<ConsensusEvent>>, ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            // check to see if a service with the given service_id exists
            scabbard_service::table
                .filter(
                    scabbard_service::circuit_id
                        .eq(service_id.circuit_id().to_string())
                        .and(scabbard_service::service_id.eq(service_id.service_id().to_string())),
                )
                .first::<ScabbardServiceModel>(self.conn)
                .optional()
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?
                .ok_or_else(|| {
                    ScabbardStoreError::InvalidState(InvalidStateError::with_message(String::from(
                        "Service does not exist",
                    )))
                })?;

            let consensus_events = consensus_2pc_event::table
                .filter(
                    consensus_2pc_event::circuit_id
                        .eq(service_id.circuit_id().to_string())
                        .and(
                            consensus_2pc_event::service_id
                                .eq(service_id.service_id().to_string())
                                .and(consensus_2pc_event::executed_at.is_null()),
                        ),
                )
                .order(consensus_2pc_event::id.desc())
                .select((consensus_2pc_event::id, consensus_2pc_event::event_type))
                .load::<(i64, EventTypeModel)>(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            let event_ids = consensus_events
                .clone()
                .into_iter()
                .map(|(id, _)| id)
                .collect::<Vec<_>>();

            let mut all_events = Vec::new();

            let mut alarm_events = consensus_events
                .into_iter()
                .filter_map(|(id, event_type)| match event_type {
                    EventTypeModel::Alarm => Some(Identified {
                        id,
                        record: ConsensusEvent::TwoPhaseCommit(Event::Alarm()),
                    }),
                    _ => None,
                })
                .collect::<Vec<Identified<ConsensusEvent>>>();

            all_events.append(&mut alarm_events);

            let deliver_events = consensus_2pc_deliver_event::table
                .filter(consensus_2pc_deliver_event::event_id.eq_any(&event_ids))
                .load::<Consensus2pcDeliverEventModel>(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            let start_events = consensus_2pc_start_event::table
                .filter(consensus_2pc_start_event::event_id.eq_any(&event_ids))
                .load::<Consensus2pcStartEventModel>(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            let vote_events = consensus_2pc_vote_event::table
                .filter(consensus_2pc_vote_event::event_id.eq_any(&event_ids))
                .load::<Consensus2pcVoteEventModel>(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            for deliver in deliver_events {
                let process = ServiceId::new(deliver.receiver_service_id).map_err(|err| {
                    ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                })?;

                let message = match deliver.message_type {
                    MessageTypeModel::VoteResponse => {
                        let vote_response = deliver
                            .vote_response
                            .map(|v| match v.as_str() {
                                "TRUE" => Some(true),
                                "FALSE" => Some(false),
                                _ => None,
                            })
                            .ok_or_else(|| {
                                ScabbardStoreError::Internal(InternalError::with_message(
                                    "Failed to get vote response for message in 'deliver' \
                                    event, no associated vote response found"
                                        .to_string(),
                                ))
                            })?
                            .ok_or_else(|| {
                                ScabbardStoreError::Internal(InternalError::with_message(
                                    "Failed to get 'vote response' for message in 'deliver' event, \
                                    invalid vote response found"
                                    .to_string(),
                                ))
                            })?;
                        Message::VoteResponse(deliver.epoch as u64, vote_response)
                    }
                    MessageTypeModel::DecisionRequest => {
                        Message::DecisionRequest(deliver.epoch as u64)
                    }
                    MessageTypeModel::VoteRequest => Message::VoteRequest(
                        deliver.epoch as u64,
                        deliver.vote_request.ok_or_else(|| {
                            ScabbardStoreError::Internal(InternalError::with_message(
                                "Failed to list events, deliver event has message type 'vote \
                                request' but no associated value"
                                    .to_string(),
                            ))
                        })?,
                    ),
                    MessageTypeModel::Commit => Message::Commit(deliver.epoch as u64),
                    MessageTypeModel::Abort => Message::Abort(deliver.epoch as u64),
                    MessageTypeModel::DecisionAck => Message::DecisionAck(deliver.epoch as u64),
                };

                let event = Identified {
                    id: deliver.event_id,
                    record: ConsensusEvent::TwoPhaseCommit(Event::Deliver(process, message)),
                };
                all_events.push(event);
            }

            for start in start_events {
                let event = Identified {
                    id: start.event_id,
                    record: ConsensusEvent::TwoPhaseCommit(Event::Start(start.value)),
                };
                all_events.push(event);
            }

            for vote in vote_events {
                let vote_decision = match vote.vote.as_str() {
                    "TRUE" => true,
                    "FALSE" => false,
                    _ => {
                        return Err(ScabbardStoreError::InvalidState(
                            InvalidStateError::with_message(
                                "Failed to list consensus events, invalid vote found".to_string(),
                            ),
                        ))
                    }
                };
                let event = Identified {
                    id: vote.event_id,
                    record: ConsensusEvent::TwoPhaseCommit(Event::Vote(vote_decision)),
                };
                all_events.push(event);
            }

            all_events.sort_by(|a, b| a.id.cmp(&b.id));

            Ok(all_events)
        })
    }
}
