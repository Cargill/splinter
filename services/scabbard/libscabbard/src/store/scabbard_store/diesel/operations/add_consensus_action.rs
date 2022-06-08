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
use std::time::SystemTime;

#[cfg(feature = "postgres")]
use diesel::pg::PgConnection;
#[cfg(feature = "sqlite")]
use diesel::sqlite::SqliteConnection;
use diesel::{dsl::insert_into, prelude::*};
use splinter::error::{InternalError, InvalidStateError};
use splinter::service::FullyQualifiedServiceId;

use crate::store::scabbard_store::diesel::{
    models::{
        Consensus2pcContextModel, Consensus2pcNotificationModel,
        Consensus2pcSendMessageActionModel, Consensus2pcUpdateContextActionModel,
        InsertableConsensus2pcActionModel, UpdateContextActionParticipantList,
    },
    schema::{
        consensus_2pc_action, consensus_2pc_context, consensus_2pc_notification_action,
        consensus_2pc_send_message_action, consensus_2pc_update_context_action,
        consensus_2pc_update_context_action_participant,
    },
};
use crate::store::scabbard_store::ScabbardStoreError;
use crate::store::scabbard_store::{
    two_phase_commit::{Action, Message, Notification},
    ConsensusAction, ConsensusContext,
};

use super::ScabbardStoreOperations;

const OPERATION_NAME: &str = "add_consensus_action";

pub(in crate::store::scabbard_store::diesel) trait AddActionOperation {
    fn add_consensus_action(
        &self,
        action: ConsensusAction,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<i64, ScabbardStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> AddActionOperation for ScabbardStoreOperations<'a, SqliteConnection> {
    fn add_consensus_action(
        &self,
        action: ConsensusAction,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<i64, ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            let ConsensusAction::TwoPhaseCommit(action) = action;

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
                        "Cannot add consensus action, context with service ID {} does not exist",
                        service_id,
                    )))
                })?;

            let position = consensus_2pc_action::table
                .filter(consensus_2pc_action::service_id.eq(format!("{}", service_id)))
                .order(consensus_2pc_action::position.desc())
                .select(consensus_2pc_action::position)
                .first::<i32>(self.conn)
                .optional()
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?
                .unwrap_or(0)
                + 1;

            let insertable_action = InsertableConsensus2pcActionModel {
                service_id: format!("{}", service_id),
                executed_at: None,
                position,
            };

            insert_into(consensus_2pc_action::table)
                .values(vec![insertable_action])
                .execute(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;
            let action_id = consensus_2pc_action::table
                .order(consensus_2pc_action::id.desc())
                .select(consensus_2pc_action::id)
                .first::<i64>(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            match action {
                Action::Update(context, alarm) => match context {
                    ConsensusContext::TwoPhaseCommit(context) => {
                        let action_alarm = get_timestamp(alarm)?;

                        let update_context_action = Consensus2pcUpdateContextActionModel::try_from(
                            (&context, &action_id, &action_alarm),
                        )?;

                        insert_into(consensus_2pc_update_context_action::table)
                            .values(vec![update_context_action])
                            .execute(self.conn)
                            .map_err(|err| {
                                ScabbardStoreError::from_source_with_operation(
                                    err,
                                    OPERATION_NAME.to_string(),
                                )
                            })?;

                        let participants =
                            UpdateContextActionParticipantList::try_from((&context, &action_id))?
                                .inner;
                        insert_into(consensus_2pc_update_context_action_participant::table)
                            .values(participants)
                            .execute(self.conn)
                            .map_err(|err| {
                                ScabbardStoreError::from_source_with_operation(
                                    err,
                                    OPERATION_NAME.to_string(),
                                )
                            })?;

                        Ok(action_id)
                    }
                },
                Action::SendMessage(receiving_process, message) => {
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

                    let send_message_action = Consensus2pcSendMessageActionModel {
                        action_id,
                        epoch: i64::try_from(epoch)
                            .map_err(|err| InternalError::from_source(Box::new(err)))?,
                        receiver_service_id: format!("{}", receiving_process),
                        message_type,
                        vote_response,
                        vote_request,
                    };
                    insert_into(consensus_2pc_send_message_action::table)
                        .values(vec![send_message_action])
                        .execute(self.conn)
                        .map_err(|err| {
                            ScabbardStoreError::from_source_with_operation(
                                err,
                                OPERATION_NAME.to_string(),
                            )
                        })?;
                    Ok(action_id)
                }
                Action::Notify(notification) => {
                    let (notification_type, dropped_message, request_for_vote_value) =
                        match &notification {
                            Notification::MessageDropped(message) => {
                                (String::from(&notification), Some(message.clone()), None)
                            }
                            Notification::ParticipantRequestForVote(value) => {
                                (String::from(&notification), None, Some(value.clone()))
                            }
                            _ => (String::from(&notification), None, None),
                        };

                    let notification_action = Consensus2pcNotificationModel {
                        action_id,
                        notification_type,
                        dropped_message,
                        request_for_vote_value,
                    };
                    insert_into(consensus_2pc_notification_action::table)
                        .values(vec![notification_action])
                        .execute(self.conn)
                        .map_err(|err| {
                            ScabbardStoreError::from_source_with_operation(
                                err,
                                OPERATION_NAME.to_string(),
                            )
                        })?;
                    Ok(action_id)
                }
            }
        })
    }
}

#[cfg(feature = "postgres")]
impl<'a> AddActionOperation for ScabbardStoreOperations<'a, PgConnection> {
    fn add_consensus_action(
        &self,
        action: ConsensusAction,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<i64, ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            let ConsensusAction::TwoPhaseCommit(action) = action;

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

            let position = consensus_2pc_action::table
                .filter(consensus_2pc_action::service_id.eq(format!("{}", service_id)))
                .order(consensus_2pc_action::position.desc())
                .select(consensus_2pc_action::position)
                .first::<i32>(self.conn)
                .optional()
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?
                .unwrap_or(0)
                + 1;

            let insertable_action = InsertableConsensus2pcActionModel {
                service_id: format!("{}", service_id),
                executed_at: None,
                position,
            };

            let action_id: i64 = insert_into(consensus_2pc_action::table)
                .values(vec![insertable_action])
                .returning(consensus_2pc_action::id)
                .get_result(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            match action {
                Action::Update(context, alarm) => match context {
                    ConsensusContext::TwoPhaseCommit(context) => {
                        let action_alarm = get_timestamp(alarm)?;

                        let update_context_action = Consensus2pcUpdateContextActionModel::try_from(
                            (&context, &action_id, &action_alarm),
                        )?;

                        insert_into(consensus_2pc_update_context_action::table)
                            .values(vec![update_context_action])
                            .execute(self.conn)
                            .map_err(|err| {
                                ScabbardStoreError::from_source_with_operation(
                                    err,
                                    OPERATION_NAME.to_string(),
                                )
                            })?;

                        let participants =
                            UpdateContextActionParticipantList::try_from((&context, &action_id))?
                                .inner;
                        insert_into(consensus_2pc_update_context_action_participant::table)
                            .values(participants)
                            .execute(self.conn)
                            .map_err(|err| {
                                ScabbardStoreError::from_source_with_operation(
                                    err,
                                    OPERATION_NAME.to_string(),
                                )
                            })?;

                        Ok(action_id)
                    }
                },
                Action::SendMessage(receiving_process, message) => {
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

                    let send_message_action = Consensus2pcSendMessageActionModel {
                        action_id,
                        epoch: i64::try_from(epoch)
                            .map_err(|err| InternalError::from_source(Box::new(err)))?,
                        receiver_service_id: format!("{}", receiving_process),
                        message_type,
                        vote_response,
                        vote_request,
                    };
                    insert_into(consensus_2pc_send_message_action::table)
                        .values(vec![send_message_action])
                        .execute(self.conn)
                        .map_err(|err| {
                            ScabbardStoreError::from_source_with_operation(
                                err,
                                OPERATION_NAME.to_string(),
                            )
                        })?;
                    Ok(action_id)
                }
                Action::Notify(notification) => {
                    let (notification_type, dropped_message, request_for_vote_value) =
                        match &notification {
                            Notification::MessageDropped(message) => {
                                (String::from(&notification), Some(message.clone()), None)
                            }
                            Notification::ParticipantRequestForVote(value) => {
                                (String::from(&notification), None, Some(value.clone()))
                            }
                            _ => (String::from(&notification), None, None),
                        };

                    let notification_action = Consensus2pcNotificationModel {
                        action_id,
                        notification_type,
                        dropped_message,
                        request_for_vote_value,
                    };
                    insert_into(consensus_2pc_notification_action::table)
                        .values(vec![notification_action])
                        .execute(self.conn)
                        .map_err(|err| {
                            ScabbardStoreError::from_source_with_operation(
                                err,
                                OPERATION_NAME.to_string(),
                            )
                        })?;
                    Ok(action_id)
                }
            }
        })
    }
}

fn get_timestamp(time: Option<SystemTime>) -> Result<Option<i64>, ScabbardStoreError> {
    match time {
        Some(time) => Ok(Some(
            i64::try_from(
                time.duration_since(SystemTime::UNIX_EPOCH)
                    .map_err(|err| {
                        ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                    })?
                    .as_secs(),
            )
            .map_err(|err| {
                ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
            })?,
        )),
        None => Ok(None),
    }
}
