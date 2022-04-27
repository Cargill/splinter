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
use std::convert::TryFrom;

use diesel::prelude::*;
use splinter::error::{InternalError, InvalidStateError};
use splinter::service::FullyQualifiedServiceId;
use splinter::service::ServiceId;

use crate::store::scabbard_store::diesel::{
    models::{
        Consensus2pcCoordinatorContextModel, Consensus2pcParticipantContextModel,
        TwoPcConsensusDeliverEventModel, TwoPcConsensusStartEventModel,
        TwoPcConsensusVoteEventModel,
    },
    schema::{
        consensus_2pc_coordinator_context, consensus_2pc_participant_context,
        two_pc_consensus_deliver_event, two_pc_consensus_event, two_pc_consensus_start_event,
        two_pc_consensus_vote_event,
    },
};
use crate::store::scabbard_store::ScabbardStoreError;
use crate::store::scabbard_store::{
    event::ReturnedScabbardConsensusEvent,
    two_phase::{event::Scabbard2pcEvent, message::Scabbard2pcMessage},
};

use super::ScabbardStoreOperations;

pub(in crate::store::scabbard_store::diesel) trait ListEventsOperation {
    fn list_consensus_events(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<Vec<ReturnedScabbardConsensusEvent>, ScabbardStoreError>;
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
        epoch: u64,
    ) -> Result<Vec<ReturnedScabbardConsensusEvent>, ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            let epoch = i64::try_from(epoch).map_err(|err| {
                ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
            })?;
            // check to see if a coordinator context with the given epoch and service_id exists
            let coordinator_context = consensus_2pc_coordinator_context::table
                .filter(consensus_2pc_coordinator_context::epoch.eq(epoch).and(
                    consensus_2pc_coordinator_context::service_id.eq(format!("{}", service_id)),
                ))
                .first::<Consensus2pcCoordinatorContextModel>(self.conn)
                .optional()?;

            // check to see if a participant context with the given epoch and service_id exists
            let participant_context = consensus_2pc_participant_context::table
                .filter(consensus_2pc_participant_context::epoch.eq(epoch).and(
                    consensus_2pc_participant_context::service_id.eq(format!("{}", service_id)),
                ))
                .first::<Consensus2pcParticipantContextModel>(self.conn)
                .optional()?;

            let consensus_events = two_pc_consensus_event::table
                .filter(
                    two_pc_consensus_event::service_id
                        .eq(format!("{}", service_id))
                        .and(two_pc_consensus_event::epoch.eq(epoch))
                        .and(two_pc_consensus_event::executed_at.is_null()),
                )
                .order(two_pc_consensus_event::position.desc())
                .select((
                    two_pc_consensus_event::id,
                    two_pc_consensus_event::position,
                    two_pc_consensus_event::event_type,
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
                        ReturnedScabbardConsensusEvent::Scabbard2pcConsensusEvent(
                            id,
                            Scabbard2pcEvent::Alarm(),
                        ),
                    )),
                    _ => None,
                })
                .collect::<Vec<(i32, ReturnedScabbardConsensusEvent)>>();

            all_events.append(&mut alarm_events);

            let deliver_events = two_pc_consensus_deliver_event::table
                .filter(two_pc_consensus_deliver_event::event_id.eq_any(&event_ids))
                .load::<TwoPcConsensusDeliverEventModel>(self.conn)?;

            let start_events = two_pc_consensus_start_event::table
                .filter(two_pc_consensus_start_event::event_id.eq_any(&event_ids))
                .load::<TwoPcConsensusStartEventModel>(self.conn)?;

            let vote_events = two_pc_consensus_vote_event::table
                .filter(two_pc_consensus_vote_event::event_id.eq_any(&event_ids))
                .load::<TwoPcConsensusVoteEventModel>(self.conn)?;

            if coordinator_context.is_some() {
                // return an error if there is both a coordinator and a participant context for the
                // given service_id and epoch
                if participant_context.is_some() {
                    return Err(ScabbardStoreError::InvalidState(
                        InvalidStateError::with_message(format!(
                            "Failed to list events, contexts found for both participant and
                            coordinator with service_id: {} epoch: {} ",
                            service_id, epoch
                        )),
                    ));
                }

                for deliver in deliver_events {
                    let position = events_map.get(&deliver.event_id).ok_or_else(|| {
                        ScabbardStoreError::Internal(InternalError::with_message(
                            "Failed to list consensus events, invalid event ID".to_string(),
                        ))
                    })?;
                    let process = ServiceId::new(deliver.receiver_service_id).map_err(|err| {
                        ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                    })?;

                    let message = match deliver.message_type.as_str() {
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
                                        "Failed to get vote response for message in 'deliver' 
                                        event, no associated vote response found"
                                            .to_string(),
                                    ))
                                })?
                                .ok_or_else(|| {
                                    ScabbardStoreError::Internal(InternalError::with_message(
                                "Failed to get 'vote response' for message in 'deliver' event, 
                                invalid vote response found".to_string(),
                            ))
                                })?;
                            Scabbard2pcMessage::VoteResponse(deliver.epoch as u64, vote_response)
                        }
                        "DECISIONREQUEST" => {
                            Scabbard2pcMessage::DecisionRequest(deliver.epoch as u64)
                        }
                        _ => {
                            return Err(ScabbardStoreError::InvalidState(
                                InvalidStateError::with_message(
                                    "Failed to list events, invalid message type found for deliver 
                                    event"
                                        .to_string(),
                                ),
                            ))
                        }
                    };
                    let event = ReturnedScabbardConsensusEvent::Scabbard2pcConsensusEvent(
                        deliver.event_id,
                        Scabbard2pcEvent::Deliver(process, message),
                    );
                    all_events.push((*position, event));
                }

                for start in start_events {
                    let position = events_map.get(&start.event_id).ok_or_else(|| {
                        ScabbardStoreError::Internal(InternalError::with_message(
                            "Failed to list consensus events, invalid event ID".to_string(),
                        ))
                    })?;
                    let event = ReturnedScabbardConsensusEvent::Scabbard2pcConsensusEvent(
                        start.event_id,
                        Scabbard2pcEvent::Start(start.value),
                    );
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
                                    "Failed to list consensus events, 
                                invalid vote found"
                                        .to_string(),
                                ),
                            ))
                        }
                    };
                    let event = ReturnedScabbardConsensusEvent::Scabbard2pcConsensusEvent(
                        vote.event_id,
                        Scabbard2pcEvent::Vote(vote_decision),
                    );
                    all_events.push((*position, event));
                }
            } else if participant_context.is_some() {
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
                            "VOTEREQUEST" => Scabbard2pcMessage::VoteRequest(
                                deliver.epoch as u64,
                                deliver.vote_request.ok_or_else(|| {
                                    ScabbardStoreError::Internal(InternalError::with_message(
                                    "Failed to list events, deliver event has message type 'vote 
                                    request' but no associated value"
                                    .to_string(),
                                ))
                                })?,
                            ),
                            "COMMIT" => Scabbard2pcMessage::Commit(deliver.epoch as u64),
                            "ABORT" => Scabbard2pcMessage::Abort(deliver.epoch as u64),
                            "DECISIONREQUEST" => {
                                Scabbard2pcMessage::DecisionRequest(deliver.epoch as u64)
                            }
                            _ => return Err(ScabbardStoreError::InvalidState(
                                InvalidStateError::with_message(
                                    "Failed to list events, invalid message type found for deliver 
                                    event"
                                        .to_string(),
                                ),
                            )),
                        };
                    let event = ReturnedScabbardConsensusEvent::Scabbard2pcConsensusEvent(
                        deliver.event_id,
                        Scabbard2pcEvent::Deliver(process, message),
                    );
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
                                    "Failed to list consensus events, 
                                invalid vote found"
                                        .to_string(),
                                ),
                            ))
                        }
                    };
                    let event = ReturnedScabbardConsensusEvent::Scabbard2pcConsensusEvent(
                        vote.event_id,
                        Scabbard2pcEvent::Vote(vote_decision),
                    );
                    all_events.push((*position, event));
                }
            } else {
                return Err(ScabbardStoreError::InvalidState(
                    InvalidStateError::with_message(format!(
                        "Failed to list events, a context with service_id: {} and epoch: {} does 
                        not exist in ScabbardStore",
                        service_id, epoch
                    )),
                ));
            }

            all_events.sort_by(|a, b| a.0.cmp(&b.0));

            Ok(all_events.into_iter().map(|(_, event)| event).collect())
        })
    }
}
