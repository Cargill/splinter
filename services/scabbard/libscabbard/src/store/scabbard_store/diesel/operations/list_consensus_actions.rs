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
        Consensus2pcContextModel, Consensus2pcNotificationModel,
        Consensus2pcSendMessageActionModel, Consensus2pcUpdateContextActionModel,
    },
    schema::{
        consensus_2pc_action, consensus_2pc_context, consensus_2pc_notification_action,
        consensus_2pc_send_message_action, consensus_2pc_update_context_action,
        consensus_2pc_update_context_action_participant,
    },
};
use crate::store::scabbard_store::ScabbardStoreError;
use crate::store::scabbard_store::{
    two_phase::{Action, ContextBuilder, Message, Notification, Participant, State},
    ConsensusContext, IdentifiedConsensusAction,
};

use super::ScabbardStoreOperations;

pub(in crate::store::scabbard_store::diesel) trait ListActionsOperation {
    fn list_consensus_actions(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<Vec<IdentifiedConsensusAction>, ScabbardStoreError>;
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
    ) -> Result<Vec<IdentifiedConsensusAction>, ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            let epoch = i64::try_from(epoch).map_err(|err| {
                ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
            })?;
            // check to see if a context with the given epoch and service_id exists
            consensus_2pc_context::table
                .filter(
                    consensus_2pc_context::epoch
                        .eq(epoch)
                        .and(consensus_2pc_context::service_id.eq(format!("{}", service_id))),
                )
                .first::<Consensus2pcContextModel>(self.conn)
                .optional()?
                .ok_or_else(|| {
                    ScabbardStoreError::InvalidState(InvalidStateError::with_message(format!(
                        "Context with service ID {} and epoch {} does not exist",
                        service_id, epoch,
                    )))
                })?;

            let all_actions = consensus_2pc_action::table
                .filter(
                    consensus_2pc_action::service_id
                        .eq(format!("{}", service_id))
                        .and(consensus_2pc_action::epoch.eq(epoch))
                        .and(consensus_2pc_action::executed_at.is_null()),
                )
                .order(consensus_2pc_action::position.desc())
                .select((consensus_2pc_action::id, consensus_2pc_action::position))
                .load::<(i64, i32)>(self.conn)?;

            let action_ids = all_actions
                .clone()
                .into_iter()
                .map(|(id, _)| id)
                .collect::<Vec<_>>();
            let actions_map: HashMap<_, _> = all_actions.into_iter().collect();

            let mut all_actions = Vec::new();

            let update_context_actions = consensus_2pc_update_context_action::table
                .filter(consensus_2pc_update_context_action::action_id.eq_any(&action_ids))
                .load::<Consensus2pcUpdateContextActionModel>(self.conn)?;

            let send_message_actions = consensus_2pc_send_message_action::table
                .filter(consensus_2pc_send_message_action::action_id.eq_any(&action_ids))
                .load::<Consensus2pcSendMessageActionModel>(self.conn)?;

            let notification_actions = consensus_2pc_notification_action::table
                .filter(consensus_2pc_notification_action::action_id.eq_any(&action_ids))
                .load::<Consensus2pcNotificationModel>(self.conn)?;

            for update_context in update_context_actions {
                let position = actions_map.get(&update_context.action_id).ok_or_else(|| {
                    ScabbardStoreError::Internal(InternalError::with_message(
                        "Failed to list consensus actions, error finding action ID".to_string(),
                    ))
                })?;

                let participants = consensus_2pc_update_context_action_participant::table
                    .filter(
                        consensus_2pc_update_context_action_participant::action_id
                            .eq(update_context.action_id),
                    )
                    .select((
                        consensus_2pc_update_context_action_participant::process,
                        consensus_2pc_update_context_action_participant::vote,
                    ))
                    .load::<(String, Option<String>)>(self.conn)?;

                let mut final_participants = Vec::new();

                for (service_id, vote) in participants.into_iter() {
                    let vote = vote
                        .map(|v| match v.as_str() {
                            "TRUE" => Ok(true),
                            "FALSE" => Ok(false),
                            _ => Err(ScabbardStoreError::Internal(InternalError::with_message(
                                "Failed to get 'vote response' send message action, invalid \
                                    vote response found"
                                    .to_string(),
                            ))),
                        })
                        .transpose()?;
                    let process = ServiceId::new(service_id).map_err(|err| {
                        ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                    })?;
                    final_participants.push(Participant { process, vote });
                }

                let state = match update_context.state.as_str() {
                    "WAITINGFORSTART" => State::WaitingForStart,
                    "VOTING" => {
                        let vote_timeout_start = get_system_time(
                            update_context.vote_timeout_start,
                        )?
                        .ok_or_else(|| {
                            ScabbardStoreError::Internal(InternalError::with_message(
                                "failed to get update context action with status 'voting', \
                                    no vote timeout start time set"
                                    .to_string(),
                            ))
                        })?;
                        State::Voting { vote_timeout_start }
                    }
                    "WAITINGFORVOTE" => State::WaitingForVote,
                    "ABORT" => State::Abort,
                    "COMMIT" => State::Commit,
                    "VOTED" => {
                        let decision_timeout_start = get_system_time(
                            update_context.decision_timeout_start,
                        )?
                        .ok_or_else(|| {
                            ScabbardStoreError::Internal(InternalError::with_message(
                                "Failed to list actions, context has state 'Voted' but no decision \
                                timeout start time set"
                                .to_string(),
                            ))
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
                                    "Failed to get update context action, context has state \
                                    'voted' but no associated vote response found"
                                        .to_string(),
                                ))
                            })?
                            .ok_or_else(|| {
                                ScabbardStoreError::Internal(InternalError::with_message(
                                    "Failed to get update context action, invalid vote response \
                                    found"
                                        .to_string(),
                                ))
                            })?;
                        State::Voted {
                            vote,
                            decision_timeout_start,
                        }
                    }
                    "WAITINGFORVOTEREQUEST" => State::WaitingForVoteRequest,
                    _ => {
                        return Err(ScabbardStoreError::Internal(InternalError::with_message(
                            "Failed to list actions, invalid state value found".to_string(),
                        )))
                    }
                };

                let mut context = ContextBuilder::default()
                    .with_alarm(get_system_time(update_context.alarm)?.ok_or_else(|| {
                        ScabbardStoreError::Internal(InternalError::with_message(
                            "failed to get update context action with status 'voting', no vote \
                            timeout start time set"
                                .to_string(),
                        ))
                    })?)
                    .with_coordinator(&ServiceId::new(&update_context.coordinator).map_err(
                        |err| {
                            ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                        },
                    )?)
                    .with_epoch(update_context.epoch as u64)
                    .with_participants(final_participants)
                    .with_state(state)
                    .with_this_process(
                        FullyQualifiedServiceId::new_from_string(update_context.service_id)
                            .map_err(|err| {
                                ScabbardStoreError::Internal(InternalError::from_source(Box::new(
                                    err,
                                )))
                            })?
                            .service_id(),
                    );

                if let Some(last_commit_epoch) = update_context.last_commit_epoch {
                    context = context.with_last_commit_epoch(last_commit_epoch as u64)
                };

                let context = context.build().map_err(|err| {
                    ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                })?;

                let action_alarm = get_system_time(update_context.action_alarm)?;
                let action = IdentifiedConsensusAction::TwoPhaseCommit(
                    update_context.action_id,
                    Action::Update(ConsensusContext::TwoPhaseCommit(context), action_alarm),
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
                                    "Failed to get 'vote response' send message action, no \
                                    associated vote response found"
                                        .to_string(),
                                ))
                            })?
                            .ok_or_else(|| {
                                ScabbardStoreError::Internal(InternalError::with_message(
                                    "Failed to get 'vote response' send message action, invalid \
                                    vote response found"
                                        .to_string(),
                                ))
                            })?;
                        Message::VoteResponse(send_message.epoch as u64, vote_response)
                    }
                    "DECISIONREQUEST" => Message::DecisionRequest(send_message.epoch as u64),
                    "VOTEREQUEST" => Message::VoteRequest(
                        send_message.epoch as u64,
                        send_message.vote_request.ok_or_else(|| {
                            ScabbardStoreError::Internal(InternalError::with_message(
                                "Failed to get 'vote request' send message action, no \
                                associated value found"
                                    .to_string(),
                            ))
                        })?,
                    ),
                    "COMMIT" => Message::Commit(send_message.epoch as u64),
                    "ABORT" => Message::Abort(send_message.epoch as u64),
                    _ => {
                        return Err(ScabbardStoreError::InvalidState(
                            InvalidStateError::with_message(
                                "Failed to list actions, invalid message type found".to_string(),
                            ),
                        ))
                    }
                };
                let action = IdentifiedConsensusAction::TwoPhaseCommit(
                    send_message.action_id,
                    Action::SendMessage(service_id, message),
                );
                all_actions.push((position, action));
            }

            for notification in notification_actions {
                let position = actions_map.get(&notification.action_id).ok_or_else(|| {
                    ScabbardStoreError::Internal(InternalError::with_message(
                        "Failed to list consensus actions, error finding action ID".to_string(),
                    ))
                })?;
                let notification_action = match notification.notification_type.as_str() {
                    "REQUESTFORSTART" => Notification::RequestForStart(),
                    "COORDINATORREQUESTFORVOTE" => Notification::CoordinatorRequestForVote(),
                    "PARTICIPANTREQUESTFORVOTE" => Notification::ParticipantRequestForVote(
                        notification.request_for_vote_value.ok_or_else(|| {
                            ScabbardStoreError::Internal(InternalError::with_message(
                                "Failed to get 'request for vote' notification action, no \
                                    associated value"
                                    .to_string(),
                            ))
                        })?,
                    ),
                    "COMMIT" => Notification::Commit(),
                    "ABORT" => Notification::Abort(),
                    "MESSAGEDROPPED" => Notification::MessageDropped(
                        notification.dropped_message.ok_or_else(|| {
                            ScabbardStoreError::Internal(InternalError::with_message(
                                "Failed to get 'message dropped' notification action, no \
                                associated dropped message found"
                                    .to_string(),
                            ))
                        })?,
                    ),
                    _ => {
                        return Err(ScabbardStoreError::Internal(InternalError::with_message(
                            "Failed to list actions, invalid notification type found".to_string(),
                        )))
                    }
                };
                let action = IdentifiedConsensusAction::TwoPhaseCommit(
                    notification.action_id,
                    Action::Notify(notification_action),
                );
                all_actions.push((position, action));
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
