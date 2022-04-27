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
use std::time::Duration;
use std::time::SystemTime;

use diesel::prelude::*;
use splinter::error::{InternalError, InvalidStateError};
use splinter::service::FullyQualifiedServiceId;
use splinter::service::ServiceId;

use crate::store::scabbard_store::diesel::{
    models::{
        Consensus2pcCoordinatorContextModel, Consensus2pcCoordinatorNotificationModel,
        Consensus2pcCoordinatorSendMessageActionModel, Consensus2pcParticipantContextModel,
        Consensus2pcParticipantNotificationModel, Consensus2pcParticipantSendMessageActionModel,
        Consensus2pcUpdateCoordinatorContextActionModel,
        Consensus2pcUpdateParticipantContextActionModel,
    },
    schema::{
        consensus_action, consensus_coordinator_context, consensus_coordinator_notification_action,
        consensus_coordinator_send_message_action, consensus_participant_context,
        consensus_participant_notification_action, consensus_participant_send_message_action,
        consensus_update_coordinator_context_action,
        consensus_update_coordinator_context_action_participant,
        consensus_update_participant_context_action,
        consensus_update_participant_context_action_participant,
    },
};
use crate::store::scabbard_store::ScabbardStoreError;
use crate::store::scabbard_store::{
    context::{ContextBuilder, Participant},
    state::Scabbard2pcState,
    two_phase::{
        action::{ConsensusAction, ConsensusActionNotification},
        message::Scabbard2pcMessage,
    },
    IdentifiedScabbardConsensusAction, ScabbardContext,
};

use super::ScabbardStoreOperations;

pub(in crate::store::scabbard_store::diesel) trait ListActionsOperation {
    fn list_consensus_actions(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<Vec<IdentifiedScabbardConsensusAction>, ScabbardStoreError>;
}

impl<'a, C> ListActionsOperation for ScabbardStoreOperations<'a, C>
where
    C: diesel::Connection,
    i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, C::Backend>,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
    Vec<u8>: diesel::deserialize::FromSql<diesel::sql_types::Binary, C::Backend>,
{
    fn list_consensus_actions(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<Vec<IdentifiedScabbardConsensusAction>, ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            let epoch = i64::try_from(epoch).map_err(|err| {
                ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
            })?;
            // check to see if a coordinator context with the given epoch and service_id exists
            let coordinator_context =
                consensus_coordinator_context::table
                    .filter(consensus_coordinator_context::epoch.eq(epoch).and(
                        consensus_coordinator_context::service_id.eq(format!("{}", service_id)),
                    ))
                    .first::<Consensus2pcCoordinatorContextModel>(self.conn)
                    .optional()?;

            // check to see if a participant context with the given epoch and service_id exists
            let participant_context =
                consensus_participant_context::table
                    .filter(consensus_participant_context::epoch.eq(epoch).and(
                        consensus_participant_context::service_id.eq(format!("{}", service_id)),
                    ))
                    .first::<Consensus2pcParticipantContextModel>(self.conn)
                    .optional()?;

            let all_actions = consensus_action::table
                .filter(
                    consensus_action::service_id
                        .eq(format!("{}", service_id))
                        .and(consensus_action::epoch.eq(epoch))
                        .and(consensus_action::executed_at.is_null()),
                )
                .order(consensus_action::position.desc())
                .select((consensus_action::id, consensus_action::position))
                .load::<(i64, i32)>(self.conn)?;

            let action_ids = all_actions
                .clone()
                .into_iter()
                .map(|(id, _)| id)
                .collect::<Vec<_>>();
            let actions_map: HashMap<_, _> = all_actions.into_iter().collect();

            let mut all_actions = Vec::new();

            if coordinator_context.is_some() {
                // return an error if there is both a coordinator and a participant context for the
                // given service_id and epoch
                if participant_context.is_some() {
                    return Err(ScabbardStoreError::InvalidState(
                        InvalidStateError::with_message(format!(
                            "Failed to list actions, contexts found for both participant and
                            coordinator with service_id: {} epoch: {} ",
                            service_id, epoch
                        )),
                    ));
                }
                let update_context_actions = consensus_update_coordinator_context_action::table
                    .filter(
                        consensus_update_coordinator_context_action::action_id.eq_any(&action_ids),
                    )
                    .load::<Consensus2pcUpdateCoordinatorContextActionModel>(self.conn)?;

                let send_message_actions = consensus_coordinator_send_message_action::table
                    .filter(
                        consensus_coordinator_send_message_action::action_id.eq_any(&action_ids),
                    )
                    .load::<Consensus2pcCoordinatorSendMessageActionModel>(self.conn)?;

                let notification_actions = consensus_coordinator_notification_action::table
                    .filter(
                        consensus_coordinator_notification_action::action_id.eq_any(&action_ids),
                    )
                    .load::<Consensus2pcCoordinatorNotificationModel>(self.conn)?;

                for update_context in update_context_actions {
                    let position = actions_map.get(&update_context.action_id).ok_or_else(|| {
                        ScabbardStoreError::Internal(InternalError::with_message(
                            "Failed to list consensus actions, error finding action ID".to_string(),
                        ))
                    })?;

                    let participants =
                        consensus_update_coordinator_context_action_participant::table
                            .filter(
                                consensus_update_coordinator_context_action_participant::action_id
                                    .eq(update_context.action_id),
                            )
                            .select((
                                consensus_update_coordinator_context_action_participant::process,
                                consensus_update_coordinator_context_action_participant::vote,
                            ))
                            .load::<(String, Option<String>)>(self.conn)?;

                    let mut final_participants = Vec::new();

                    for (service_id, vote) in participants.into_iter() {
                        let vote = vote
                            .map(|v| {
                                match v.as_str() {
                                "TRUE" => Ok(true),
                                "FALSE" => Ok(false),
                                _ => Err(
                                    ScabbardStoreError::Internal(InternalError::with_message(
                                        "Failed to get 'vote response' send message action, invalid 
                                        vote response found".to_string(),
                                    ))
                                ),
                            }
                            })
                            .transpose()?;
                        let process = ServiceId::new(service_id).map_err(|err| {
                            ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                        })?;
                        final_participants.push(Participant { process, vote });
                    }

                    let state = match update_context.state.as_str() {
                        "WAITINGFORSTART" => Scabbard2pcState::WaitingForStart,
                        "VOTING" => {
                            let vote_timeout_start = get_system_time(
                                update_context.vote_timeout_start,
                            )?
                            .ok_or_else(|| {
                                ScabbardStoreError::Internal(InternalError::with_message(
                                    "failed to get update context action with status 'voting', 
                                        no vote timeout start time set"
                                        .to_string(),
                                ))
                            })?;
                            Scabbard2pcState::Voting { vote_timeout_start }
                        }
                        "WAITINGFORVOTE" => Scabbard2pcState::WaitingForVote,
                        "ABORT" => Scabbard2pcState::Abort,
                        "COMMIT" => Scabbard2pcState::Commit,
                        _ => {
                            return Err(ScabbardStoreError::Internal(InternalError::with_message(
                                "Failed to list actions, invalid state value found".to_string(),
                            )))
                        }
                    };

                    let mut context = ContextBuilder::default()
                        .with_alarm(get_system_time(update_context.alarm)?.ok_or_else(|| {
                            ScabbardStoreError::Internal(InternalError::with_message(
                                "failed to get update context action 
                                with status 'voting', no vote timeout start time set"
                                    .to_string(),
                            ))
                        })?)
                        .with_coordinator(&ServiceId::new(&update_context.coordinator).map_err(
                            |err| {
                                ScabbardStoreError::Internal(InternalError::from_source(Box::new(
                                    err,
                                )))
                            },
                        )?)
                        .with_epoch(update_context.epoch as u64)
                        .with_participants(final_participants)
                        .with_state(state)
                        .with_this_process(&ServiceId::new(update_context.coordinator).map_err(
                            |err| {
                                ScabbardStoreError::Internal(InternalError::from_source(Box::new(
                                    err,
                                )))
                            },
                        )?);

                    if let Some(last_commit_epoch) = update_context.last_commit_epoch {
                        context = context.with_last_commit_epoch(last_commit_epoch as u64)
                    };

                    let context = context.build().map_err(|err| {
                        ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                    })?;

                    let coordinator_action_alarm =
                        get_system_time(update_context.coordinator_action_alarm)?;
                    let action = IdentifiedScabbardConsensusAction::Scabbard2pcConsensusAction(
                        update_context.action_id,
                        ConsensusAction::Update(
                            ScabbardContext::Scabbard2pcContext(context),
                            coordinator_action_alarm,
                        ),
                    );
                    all_actions.push((position, action));
                }
                for send_message in send_message_actions {
                    let position = actions_map.get(&send_message.action_id).ok_or_else(|| {
                        ScabbardStoreError::Internal(InternalError::with_message(
                            "Failed to list consensus actions, error finding action ID".to_string(),
                        ))
                    })?;
                    let service_id =
                        ServiceId::new(send_message.receiver_service_id).map_err(|err| {
                            ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                        })?;

                    let message = match send_message.message_type.as_str() {
                        "VOTERESPONSE" => {
                            let vote_response = send_message
                                .vote_response
                                .map(|v| match v.as_str() {
                                    "TRUE" => Some(true),
                                    "FALSE" => Some(false),
                                    _ => None,
                                })
                                .ok_or_else(|| {
                                    ScabbardStoreError::Internal(InternalError::with_message(
                                        "Failed to get 'vote response' send message action, no 
                                    associated vote response found"
                                            .to_string(),
                                    ))
                                })?
                                .ok_or_else(|| {
                                    ScabbardStoreError::Internal(InternalError::with_message(
                                "Failed to get 'vote response' send message action, invalid  
                                vote response found".to_string(),
                            ))
                                })?;
                            Scabbard2pcMessage::VoteResponse(
                                send_message.epoch as u64,
                                vote_response,
                            )
                        }
                        "DECISIONREQUEST" => {
                            Scabbard2pcMessage::DecisionRequest(send_message.epoch as u64)
                        }
                        _ => {
                            return Err(ScabbardStoreError::InvalidState(
                                InvalidStateError::with_message(
                                    "Failed to list actions, invalid message type found"
                                        .to_string(),
                                ),
                            ))
                        }
                    };
                    let action = IdentifiedScabbardConsensusAction::Scabbard2pcConsensusAction(
                        send_message.action_id,
                        ConsensusAction::SendMessage(service_id, message),
                    );
                    all_actions.push((position, action));
                }

                for notification in notification_actions {
                    let position = actions_map.get(&notification.action_id).ok_or_else(|| {
                        ScabbardStoreError::Internal(InternalError::with_message(
                            "Failed to list consensus actions, error finding action ID".to_string(),
                        ))
                    })?;
                    let coordinator_notification = match notification.notification_type.as_str() {
                        "REQUESTFORSTART" => ConsensusActionNotification::RequestForStart(),
                        "COORDINATORREQUESTFORVOTE" => {
                            ConsensusActionNotification::CoordinatorRequestForVote()
                        }
                        "COMMIT" => ConsensusActionNotification::Commit(),
                        "ABORT" => ConsensusActionNotification::Abort(),
                        "MESSAGEDROPPED" => ConsensusActionNotification::MessageDropped(
                            notification.dropped_message.ok_or_else(|| {
                                ScabbardStoreError::Internal(InternalError::with_message(
                                    "Failed to get 'message dropped' notification action, no 
                                    associated dropped message found"
                                        .to_string(),
                                ))
                            })?,
                        ),
                        _ => {
                            return Err(ScabbardStoreError::Internal(InternalError::with_message(
                                "Failed to list actions, invalid notification type found"
                                    .to_string(),
                            )))
                        }
                    };
                    let action = IdentifiedScabbardConsensusAction::Scabbard2pcConsensusAction(
                        notification.action_id,
                        ConsensusAction::Notify(coordinator_notification),
                    );
                    all_actions.push((position, action));
                }
            } else if participant_context.is_some() {
                let update_context_actions = consensus_update_participant_context_action::table
                    .filter(
                        consensus_update_participant_context_action::action_id.eq_any(&action_ids),
                    )
                    .load::<Consensus2pcUpdateParticipantContextActionModel>(self.conn)?;

                let send_message_actions = consensus_participant_send_message_action::table
                    .filter(
                        consensus_participant_send_message_action::action_id.eq_any(&action_ids),
                    )
                    .load::<Consensus2pcParticipantSendMessageActionModel>(self.conn)?;

                let notification_actions = consensus_participant_notification_action::table
                    .filter(
                        consensus_participant_notification_action::action_id.eq_any(&action_ids),
                    )
                    .load::<Consensus2pcParticipantNotificationModel>(self.conn)?;

                for update_context in update_context_actions {
                    let position = actions_map.get(&update_context.action_id).ok_or_else(|| {
                        ScabbardStoreError::Internal(InternalError::with_message(
                            "Failed to list consensus actions, error finding action ID".to_string(),
                        ))
                    })?;
                    let participants =
                    consensus_update_participant_context_action_participant::table
                        .filter(
                            consensus_update_participant_context_action_participant::action_id
                            .eq(update_context.action_id)
                            .and(consensus_update_participant_context_action_participant::service_id
                                .eq(format!("{}", service_id)))
                            .and(consensus_update_participant_context_action_participant::epoch
                                .eq(epoch))
                        )
                        .select(consensus_update_participant_context_action_participant::process)
                        .load::<String>(self.conn)?
                        .into_iter()
                        .map(ServiceId::new)
                        .collect::<Result<Vec<ServiceId>, _>>().map_err(|err| {
                            ScabbardStoreError::Internal(
                                InternalError::from_source(Box::new(err))
                            )
                        })?;

                    let state = match update_context.state.as_str() {
                        "WAITINGFORVOTE" => Scabbard2pcState::WaitingForStart,
                        "VOTED" => {
                            let decision_timeout_start =
                                get_system_time(update_context.decision_timeout_start)?
                                    .ok_or_else(|| {
                                        ScabbardStoreError::Internal(
                                    InternalError::with_message(
                                        "Failed to list actions, participant has state 'Voted' but 
                                        no decision timeout start time set".to_string()
                                    )
                                )
                                    })?;
                            let vote = update_context
                                .vote
                                .map(|v| match v.as_str() {
                                    "TRUE" => Some(true),
                                    "FALSE" => Some(false),
                                    _ => None,
                                })
                                .ok_or_else(|| {
                                    ScabbardStoreError::Internal(InternalError::with_message(
                                    "Failed to get participant update context action, context has
                                    state 'voted' but no associated vote response found"
                                    .to_string(),
                                ))
                                })?
                                .ok_or_else(|| {
                                    ScabbardStoreError::Internal(InternalError::with_message(
                                    "Failed to get participant update context action, invalid vote
                                    response found".to_string(),
                            ))
                                })?;
                            Scabbard2pcState::Voted {
                                vote,
                                decision_timeout_start,
                            }
                        }
                        "WAITINGFORVOTEREQUEST" => Scabbard2pcState::WaitingForVoteRequest,
                        "ABORT" => Scabbard2pcState::Abort,
                        "COMMIT" => Scabbard2pcState::Commit,
                        _ => {
                            return Err(ScabbardStoreError::Internal(InternalError::with_message(
                                "Failed to list actions, invalid state 
                                value found"
                                    .to_string(),
                            )))
                        }
                    };

                    let mut context = ContextBuilder::default()
                        .with_alarm(get_system_time(update_context.alarm)?.ok_or_else(|| {
                            ScabbardStoreError::Internal(InternalError::with_message(
                                "failed to get update context action 
                                with status 'voting', no vote timeout start time set"
                                    .to_string(),
                            ))
                        })?)
                        .with_coordinator(&ServiceId::new(update_context.coordinator).map_err(
                            |err| {
                                ScabbardStoreError::Internal(InternalError::from_source(Box::new(
                                    err,
                                )))
                            },
                        )?)
                        .with_epoch(update_context.epoch as u64)
                        .with_participant_processes(participants)
                        .with_state(state)
                        .with_this_process(
                            FullyQualifiedServiceId::new_from_string(update_context.service_id)
                                .map_err(|err| {
                                    ScabbardStoreError::Internal(InternalError::from_source(
                                        Box::new(err),
                                    ))
                                })?
                                .service_id(),
                        );

                    if let Some(last_commit_epoch) = update_context.last_commit_epoch {
                        context = context.with_last_commit_epoch(last_commit_epoch as u64)
                    };

                    let context = context.build().map_err(|err| {
                        ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                    })?;

                    let participant_action_alarm =
                        get_system_time(update_context.participant_action_alarm)?;
                    let action = IdentifiedScabbardConsensusAction::Scabbard2pcConsensusAction(
                        update_context.action_id,
                        ConsensusAction::Update(
                            ScabbardContext::Scabbard2pcContext(context),
                            participant_action_alarm,
                        ),
                    );
                    all_actions.push((position, action));
                }
                for send_message in send_message_actions {
                    let position = actions_map.get(&send_message.action_id).ok_or_else(|| {
                        ScabbardStoreError::Internal(InternalError::with_message(
                            "Failed to list consensus actions, error finding action ID".to_string(),
                        ))
                    })?;
                    let service_id =
                        ServiceId::new(send_message.receiver_service_id).map_err(|err| {
                            ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                        })?;
                    let message = match send_message.message_type.as_str() {
                        "VOTEREQUEST" => Scabbard2pcMessage::VoteRequest(
                            send_message.epoch as u64,
                            send_message.vote_request.ok_or_else(|| {
                                ScabbardStoreError::Internal(InternalError::with_message(
                                    "Failed to get 'vote request' send message action, no 
                                        associated value found"
                                        .to_string(),
                                ))
                            })?,
                        ),
                        "COMMIT" => Scabbard2pcMessage::Commit(send_message.epoch as u64),
                        "ABORT" => Scabbard2pcMessage::Abort(send_message.epoch as u64),
                        "DECISIONREQUEST" => {
                            Scabbard2pcMessage::DecisionRequest(send_message.epoch as u64)
                        }
                        _ => {
                            return Err(ScabbardStoreError::InvalidState(
                                InvalidStateError::with_message(
                                    "Failed to list actions, invalid message type found"
                                        .to_string(),
                                ),
                            ))
                        }
                    };
                    let action = IdentifiedScabbardConsensusAction::Scabbard2pcConsensusAction(
                        send_message.action_id,
                        ConsensusAction::SendMessage(service_id, message),
                    );
                    all_actions.push((position, action));
                }

                for notification in notification_actions {
                    let position = actions_map.get(&notification.action_id).ok_or_else(|| {
                        ScabbardStoreError::Internal(InternalError::with_message(
                            "Failed to list consensus actions, error finding action ID".to_string(),
                        ))
                    })?;
                    let participant_notification = match notification.notification_type.as_str() {
                        "REQUESTFORSTART" => ConsensusActionNotification::RequestForStart(),
                        "PARTICIPANTREQUESTFORVOTE" => {
                            ConsensusActionNotification::ParticipantRequestForVote(
                                notification.request_for_vote_value.ok_or_else(|| {
                                    ScabbardStoreError::Internal(InternalError::with_message(
                                        "Failed to get 'request for vote' notification action, no 
                                        associated value"
                                            .to_string(),
                                    ))
                                })?,
                            )
                        }
                        "COMMIT" => ConsensusActionNotification::Commit(),
                        "ABORT" => ConsensusActionNotification::Abort(),
                        "MESSAGEDROPPED" => ConsensusActionNotification::MessageDropped(
                            notification.dropped_message.ok_or_else(|| {
                                ScabbardStoreError::Internal(InternalError::with_message(
                                    "Failed to get 'message dropped' notification action, no 
                                    associated dropped message found"
                                        .to_string(),
                                ))
                            })?,
                        ),
                        _ => {
                            return Err(ScabbardStoreError::Internal(InternalError::with_message(
                                "Failed to list actions, invalid notification type found"
                                    .to_string(),
                            )))
                        }
                    };
                    let action = IdentifiedScabbardConsensusAction::Scabbard2pcConsensusAction(
                        notification.action_id,
                        ConsensusAction::Notify(participant_notification),
                    );
                    all_actions.push((position, action));
                }
            } else {
                return Err(ScabbardStoreError::InvalidState(
                    InvalidStateError::with_message(format!(
                        "Faild to list actions, a context with service_id: {} and epoch: {} does 
                        not exist in ScabbardStore",
                        service_id, epoch
                    )),
                ));
            }

            all_actions.sort_by(|a, b| a.0.cmp(b.0));

            Ok(all_actions.into_iter().map(|(_, action)| action).collect())
        })
    }
}

fn get_system_time(time: Option<i64>) -> Result<Option<SystemTime>, ScabbardStoreError> {
    match time {
        Some(time) => Ok(Some(
            SystemTime::UNIX_EPOCH
                .checked_add(Duration::from_secs(time as u64))
                .ok_or_else(|| {
                    InternalError::with_message(
                        "timestamp could not be represented as a `SystemTime`".to_string(),
                    )
                })?,
        )),
        None => Ok(None),
    }
}
