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

use std::collections::HashMap;

use diesel::prelude::*;
use splinter::error::{InternalError, InvalidStateError};
use splinter::service::FullyQualifiedServiceId;
use splinter::service::ServiceId;

use crate::store::scabbard_store::diesel::{
    models::{
        Consensus2pcContextModel, Consensus2pcDeliverEventModel, Consensus2pcStartEventModel,
        Consensus2pcVoteEventModel,
    },
    schema::{
        consensus_2pc_context, consensus_2pc_deliver_event, consensus_2pc_event,
        consensus_2pc_start_event, consensus_2pc_vote_event,
    },
};
use crate::store::scabbard_store::ScabbardStoreError;
use crate::store::scabbard_store::{
    event::ConsensusEvent,
    identified::Identified,
    two_phase_commit::{Event, Message},
};

use super::ScabbardStoreOperations;

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
{
    fn list_consensus_events(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Vec<Identified<ConsensusEvent>>, ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            // check to see if a context with the given service_id exists
            consensus_2pc_context::table
                .filter(consensus_2pc_context::service_id.eq(format!("{}", service_id)))
                .first::<Consensus2pcContextModel>(self.conn)
                .optional()?
                .ok_or_else(|| {
                    ScabbardStoreError::InvalidState(InvalidStateError::with_message(format!(
                        "Context with service ID {} does not exist",
                        service_id,
                    )))
                })?;

            let consensus_events = consensus_2pc_event::table
                .filter(
                    consensus_2pc_event::service_id
                        .eq(format!("{}", service_id))
                        .and(consensus_2pc_event::executed_at.is_null()),
                )
                .order(consensus_2pc_event::position.desc())
                .select((
                    consensus_2pc_event::id,
                    consensus_2pc_event::position,
                    consensus_2pc_event::event_type,
                ))
                .load::<(i64, i32, String)>(self.conn)?;

            let event_ids = consensus_events
                .clone()
                .into_iter()
                .map(|(id, _, _)| id)
                .collect::<Vec<_>>();

            let events_map: HashMap<_, _> = consensus_events
                .clone()
                .into_iter()
                .map(|(id, position, _)| (id, position))
                .collect();

            let mut all_events = Vec::new();

            let mut alarm_events = consensus_events
                .into_iter()
                .filter_map(|(id, position, event_type)| match event_type.as_str() {
                    "ALARM" => Some((
                        position,
                        Identified {
                            id,
                            record: ConsensusEvent::TwoPhaseCommit(Event::Alarm()),
                        },
                    )),
                    _ => None,
                })
                .collect::<Vec<(i32, Identified<ConsensusEvent>)>>();

            all_events.append(&mut alarm_events);

            let deliver_events = consensus_2pc_deliver_event::table
                .filter(consensus_2pc_deliver_event::event_id.eq_any(&event_ids))
                .load::<Consensus2pcDeliverEventModel>(self.conn)?;

            let start_events = consensus_2pc_start_event::table
                .filter(consensus_2pc_start_event::event_id.eq_any(&event_ids))
                .load::<Consensus2pcStartEventModel>(self.conn)?;

            let vote_events = consensus_2pc_vote_event::table
                .filter(consensus_2pc_vote_event::event_id.eq_any(&event_ids))
                .load::<Consensus2pcVoteEventModel>(self.conn)?;

            for deliver in deliver_events {
                let position = events_map.get(&deliver.event_id).ok_or_else(|| {
                    ScabbardStoreError::Internal(InternalError::with_message(
                        "Failed to list consensus events, invalid event ID".to_string(),
                    ))
                })?;
                let process = ServiceId::new(deliver.receiver_service_id).map_err(|err| {
                    ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                })?;

                let message =
                    match deliver.message_type.as_str() {
                        "VOTERESPONSE" => {
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
                        "DECISIONREQUEST" => Message::DecisionRequest(deliver.epoch as u64),
                        "VOTEREQUEST" => Message::VoteRequest(
                            deliver.epoch as u64,
                            deliver.vote_request.ok_or_else(|| {
                                ScabbardStoreError::Internal(InternalError::with_message(
                                    "Failed to list events, deliver event has message type 'vote \
                                request' but no associated value"
                                        .to_string(),
                                ))
                            })?,
                        ),
                        "COMMIT" => Message::Commit(deliver.epoch as u64),
                        "ABORT" => Message::Abort(deliver.epoch as u64),
                        _ => return Err(ScabbardStoreError::InvalidState(
                            InvalidStateError::with_message(
                                "Failed to list events, invalid message type found for deliver \
                                event"
                                    .to_string(),
                            ),
                        )),
                    };

                let event = Identified {
                    id: deliver.event_id,
                    record: ConsensusEvent::TwoPhaseCommit(Event::Deliver(process, message)),
                };
                all_events.push((*position, event));
            }

            for start in start_events {
                let position = events_map.get(&start.event_id).ok_or_else(|| {
                    ScabbardStoreError::Internal(InternalError::with_message(
                        "Failed to list consensus events, invalid event ID".to_string(),
                    ))
                })?;
                let event = Identified {
                    id: start.event_id,
                    record: ConsensusEvent::TwoPhaseCommit(Event::Start(start.value)),
                };
                all_events.push((*position, event));
            }

            for vote in vote_events {
                let position = events_map.get(&vote.event_id).ok_or_else(|| {
                    ScabbardStoreError::Internal(InternalError::with_message(
                        "Failed to list consensus events, invalid event ID".to_string(),
                    ))
                })?;
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
                all_events.push((*position, event));
            }

            all_events.sort_by(|a, b| a.0.cmp(&b.0));

            Ok(all_events.into_iter().map(|(_, event)| event).collect())
        })
    }
}
