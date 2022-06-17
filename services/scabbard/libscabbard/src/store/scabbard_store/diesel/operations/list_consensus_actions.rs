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

use std::time::Duration;
use std::time::SystemTime;

use diesel::prelude::*;
use splinter::error::{InternalError, InvalidStateError};
use splinter::service::FullyQualifiedServiceId;
use splinter::service::ServiceId;

use crate::store::scabbard_store::diesel::{
    models::{
        Consensus2pcNotificationModel, Consensus2pcSendMessageActionModel,
        Consensus2pcUpdateContextActionModel, Consensus2pcUpdateContextActionParticipantModel,
        ScabbardServiceModel,
    },
    schema::{
        consensus_2pc_action, consensus_2pc_notification_action, consensus_2pc_send_message_action,
        consensus_2pc_update_context_action, consensus_2pc_update_context_action_participant,
        scabbard_service,
    },
};
use crate::store::scabbard_store::ScabbardStoreError;
use crate::store::scabbard_store::{
    two_phase_commit::{Action, ContextBuilder, Message, Notification, Participant, State},
    ConsensusAction, ConsensusContext, Identified,
};

use super::ScabbardStoreOperations;

const OPERATION_NAME: &str = "list_consensus_actions";

pub(in crate::store::scabbard_store::diesel) trait ListActionsOperation {
    fn list_consensus_actions(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Vec<Identified<ConsensusAction>>, ScabbardStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> ListActionsOperation for ScabbardStoreOperations<'a, SqliteConnection> {
    fn list_consensus_actions(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Vec<Identified<ConsensusAction>>, ScabbardStoreError> {
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

            let action_ids = consensus_2pc_action::table
                .filter(
                    consensus_2pc_action::circuit_id
                        .eq(service_id.circuit_id().to_string())
                        .and(
                            consensus_2pc_action::service_id
                                .eq(service_id.service_id().to_string())
                                .and(consensus_2pc_action::executed_at.is_null()),
                        ),
                )
                .order(consensus_2pc_action::id.desc())
                .select(consensus_2pc_action::id)
                .load::<i64>(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            let mut all_actions = Vec::new();

            let update_context_actions = consensus_2pc_update_context_action::table
                .filter(consensus_2pc_update_context_action::action_id.eq_any(&action_ids))
                .load::<Consensus2pcUpdateContextActionModel>(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            let send_message_actions = consensus_2pc_send_message_action::table
                .filter(consensus_2pc_send_message_action::action_id.eq_any(&action_ids))
                .load::<Consensus2pcSendMessageActionModel>(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            let notification_actions = consensus_2pc_notification_action::table
                .filter(consensus_2pc_notification_action::action_id.eq_any(&action_ids))
                .load::<Consensus2pcNotificationModel>(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            for update_context in update_context_actions {
                let participants = consensus_2pc_update_context_action_participant::table
                    .filter(
                        consensus_2pc_update_context_action_participant::action_id
                            .eq(update_context.action_id),
                    )
                    .load::<Consensus2pcUpdateContextActionParticipantModel>(self.conn)
                    .map_err(|err| {
                        ScabbardStoreError::from_source_with_operation(
                            err,
                            OPERATION_NAME.to_string(),
                        )
                    })?;

                let mut final_participants = Vec::new();

                for participant in participants.into_iter() {
                    let vote = participant
                        .vote
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
                    let process = ServiceId::new(participant.process).map_err(|err| {
                        ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                    })?;
                    final_participants.push(Participant {
                        process,
                        vote,
                        decision_ack: participant.decision_ack,
                    });
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
                    "WAITINGFORDECISIONACK" => {
                        let ack_timeout_start = get_system_time(update_context.ack_timeout_start)?
                            .ok_or_else(|| {
                                ScabbardStoreError::Internal(InternalError::with_message(
                                "Failed to list actions, context has state 'WaitingForDecisionAck' \
                                but no ack timeout start time set"
                                .to_string(),
                            ))
                            })?;
                        State::WaitingForDecisionAck { ack_timeout_start }
                    }
                    _ => {
                        return Err(ScabbardStoreError::Internal(InternalError::with_message(
                            "Failed to list actions, invalid state value found".to_string(),
                        )))
                    }
                };

                let mut context = ContextBuilder::default()
                    .with_coordinator(&ServiceId::new(&update_context.coordinator).map_err(
                        |err| {
                            ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                        },
                    )?)
                    .with_epoch(update_context.epoch as u64)
                    .with_participants(final_participants)
                    .with_state(state)
                    .with_this_process(service_id.service_id());

                if let Some(last_commit_epoch) = update_context.last_commit_epoch {
                    context = context.with_last_commit_epoch(last_commit_epoch as u64)
                };

                let context = context.build().map_err(|err| {
                    ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                })?;

                let action_alarm = get_system_time(update_context.action_alarm)?;
                let action = Identified {
                    id: update_context.action_id,
                    record: ConsensusAction::TwoPhaseCommit(Action::Update(
                        ConsensusContext::TwoPhaseCommit(context),
                        action_alarm,
                    )),
                };
                all_actions.push(action);
            }
            for send_message in send_message_actions {
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
                    "DECISION_ACK" => Message::DecisionAck(send_message.epoch as u64),
                    _ => {
                        return Err(ScabbardStoreError::InvalidState(
                            InvalidStateError::with_message(
                                "Failed to list actions, invalid message type found".to_string(),
                            ),
                        ))
                    }
                };

                let action = Identified {
                    id: send_message.action_id,
                    record: ConsensusAction::TwoPhaseCommit(Action::SendMessage(
                        service_id, message,
                    )),
                };
                all_actions.push(action);
            }

            for notification in notification_actions {
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
                let action = Identified {
                    id: notification.action_id,
                    record: ConsensusAction::TwoPhaseCommit(Action::Notify(notification_action)),
                };
                all_actions.push(action);
            }

            all_actions.sort_by(|a, b| a.id.cmp(&b.id));

            Ok(all_actions)
        })
    }
}

#[cfg(feature = "postgres")]
impl<'a> ListActionsOperation for ScabbardStoreOperations<'a, PgConnection> {
    fn list_consensus_actions(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Vec<Identified<ConsensusAction>>, ScabbardStoreError> {
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

            let action_ids = consensus_2pc_action::table
                .filter(
                    consensus_2pc_action::circuit_id
                        .eq(service_id.circuit_id().to_string())
                        .and(
                            consensus_2pc_action::service_id
                                .eq(service_id.service_id().to_string())
                                .and(consensus_2pc_action::executed_at.is_null()),
                        ),
                )
                .order(consensus_2pc_action::id.desc())
                .select(consensus_2pc_action::id)
                .load::<i64>(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            let mut all_actions = Vec::new();

            let update_context_actions = consensus_2pc_update_context_action::table
                .filter(consensus_2pc_update_context_action::action_id.eq_any(&action_ids))
                .load::<Consensus2pcUpdateContextActionModel>(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            let send_message_actions = consensus_2pc_send_message_action::table
                .filter(consensus_2pc_send_message_action::action_id.eq_any(&action_ids))
                .load::<Consensus2pcSendMessageActionModel>(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            let notification_actions = consensus_2pc_notification_action::table
                .filter(consensus_2pc_notification_action::action_id.eq_any(&action_ids))
                .load::<Consensus2pcNotificationModel>(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            for update_context in update_context_actions {
                let participants = consensus_2pc_update_context_action_participant::table
                    .filter(
                        consensus_2pc_update_context_action_participant::action_id
                            .eq(update_context.action_id),
                    )
                    .load::<Consensus2pcUpdateContextActionParticipantModel>(self.conn)
                    .map_err(|err| {
                        ScabbardStoreError::from_source_with_operation(
                            err,
                            OPERATION_NAME.to_string(),
                        )
                    })?;

                let mut final_participants = Vec::new();

                for participant in participants.into_iter() {
                    let vote = participant
                        .vote
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
                    let process = ServiceId::new(participant.process).map_err(|err| {
                        ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                    })?;
                    final_participants.push(Participant {
                        process,
                        vote,
                        decision_ack: participant.decision_ack,
                    });
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
                    .with_coordinator(&ServiceId::new(&update_context.coordinator).map_err(
                        |err| {
                            ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                        },
                    )?)
                    .with_epoch(update_context.epoch as u64)
                    .with_participants(final_participants)
                    .with_state(state)
                    .with_this_process(service_id.service_id());

                if let Some(last_commit_epoch) = update_context.last_commit_epoch {
                    context = context.with_last_commit_epoch(last_commit_epoch as u64)
                };

                let context = context.build().map_err(|err| {
                    ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                })?;

                let action_alarm = get_system_time(update_context.action_alarm)?;
                let action = Identified {
                    id: update_context.action_id,
                    record: ConsensusAction::TwoPhaseCommit(Action::Update(
                        ConsensusContext::TwoPhaseCommit(context),
                        action_alarm,
                    )),
                };
                all_actions.push(action);
            }
            for send_message in send_message_actions {
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
                    "DECISION_ACK" => Message::DecisionAck(send_message.epoch as u64),
                    _ => {
                        return Err(ScabbardStoreError::InvalidState(
                            InvalidStateError::with_message(
                                "Failed to list actions, invalid message type found".to_string(),
                            ),
                        ))
                    }
                };

                let action = Identified {
                    id: send_message.action_id,
                    record: ConsensusAction::TwoPhaseCommit(Action::SendMessage(
                        service_id, message,
                    )),
                };
                all_actions.push(action);
            }

            for notification in notification_actions {
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
                let action = Identified {
                    id: notification.action_id,
                    record: ConsensusAction::TwoPhaseCommit(Action::Notify(notification_action)),
                };
                all_actions.push(action);
            }

            all_actions.sort_by(|a, b| a.id.cmp(&b.id));

            Ok(all_actions)
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
