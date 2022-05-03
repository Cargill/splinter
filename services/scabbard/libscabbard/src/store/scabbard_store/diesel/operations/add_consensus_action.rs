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
        Consensus2pcCoordinatorContextModel, Consensus2pcCoordinatorNotificationModel,
        Consensus2pcCoordinatorSendMessageActionModel, Consensus2pcParticipantContextModel,
        Consensus2pcParticipantNotificationModel, Consensus2pcParticipantSendMessageActionModel,
        Consensus2pcUpdateCoordinatorContextActionModel,
        Consensus2pcUpdateParticipantContextActionModel, InsertableConsensus2pcActionModel,
        UpdateCoordinatorContextActionParticipantList,
        UpdateParticipantContextActionParticipantList,
    },
    schema::{
        consensus_2pc_action, consensus_2pc_coordinator_context,
        consensus_2pc_coordinator_notification_action,
        consensus_2pc_coordinator_send_message_action, consensus_2pc_participant_context,
        consensus_2pc_participant_notification_action,
        consensus_2pc_participant_send_message_action,
        consensus_2pc_update_coordinator_context_action,
        consensus_2pc_update_coordinator_context_action_participant,
        consensus_2pc_update_participant_context_action,
        consensus_2pc_update_participant_context_action_participant,
    },
};
use crate::store::scabbard_store::ScabbardStoreError;
use crate::store::scabbard_store::{
    two_phase::{
        action::{ConsensusAction, ConsensusActionNotification},
        message::Scabbard2pcMessage,
    },
    ScabbardConsensusAction, ScabbardContext,
};

use super::ScabbardStoreOperations;

pub(in crate::store::scabbard_store::diesel) trait AddActionOperation {
    fn add_consensus_action(
        &self,
        action: ScabbardConsensusAction,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<i64, ScabbardStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> AddActionOperation for ScabbardStoreOperations<'a, SqliteConnection> {
    fn add_consensus_action(
        &self,
        action: ScabbardConsensusAction,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<i64, ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            let ScabbardConsensusAction::Scabbard2pcConsensusAction(action) = action;
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

            let position = consensus_2pc_action::table
                .filter(
                    consensus_2pc_action::service_id
                        .eq(format!("{}", service_id))
                        .and(consensus_2pc_action::epoch.eq(epoch)),
                )
                .order(consensus_2pc_action::position.desc())
                .select(consensus_2pc_action::position)
                .first::<i32>(self.conn)
                .optional()?
                .unwrap_or(0)
                + 1;

            if coordinator_context.is_some() {
                // return an error if there is both a coordinator and a participant context for the
                // given service_id and epoch
                if participant_context.is_some() {
                    return Err(ScabbardStoreError::InvalidState(
                        InvalidStateError::with_message(format!(
                            "Failed to add consensus action, contexts found for 
                                participant and coordinator with service_id: {} epoch: {} ",
                            service_id, epoch
                        )),
                    ));
                }

                let insertable_action = InsertableConsensus2pcActionModel {
                    service_id: format!("{}", service_id),
                    epoch,
                    executed_at: None,
                    position,
                };

                insert_into(consensus_2pc_action::table)
                    .values(vec![insertable_action])
                    .execute(self.conn)?;
                let action_id = consensus_2pc_action::table
                    .order(consensus_2pc_action::id.desc())
                    .select(consensus_2pc_action::id)
                    .first::<i64>(self.conn)?;

                match action {
                    ConsensusAction::Update(context, alarm) => match context {
                        ScabbardContext::Scabbard2pcContext(context) => {
                            let coordinator_action_alarm = get_timestamp(alarm)?;

                            let update_context_action =
                                Consensus2pcUpdateCoordinatorContextActionModel::try_from((
                                    &context,
                                    service_id,
                                    &action_id,
                                    &coordinator_action_alarm,
                                ))?;

                            insert_into(consensus_2pc_update_coordinator_context_action::table)
                                .values(vec![update_context_action])
                                .execute(self.conn)?;

                            let participants =
                                UpdateCoordinatorContextActionParticipantList::try_from((
                                    &context, service_id, &action_id,
                                ))?
                                .inner;
                            insert_into(
                                consensus_2pc_update_coordinator_context_action_participant::table,
                            )
                            .values(participants)
                            .execute(self.conn)?;

                            Ok(action_id)
                        }
                    },
                    ConsensusAction::SendMessage(receiving_process, message) => {
                        let (message_type, vote_response) = match message {
                            Scabbard2pcMessage::DecisionRequest(_) => {
                                (String::from(&message), None)
                            }
                            Scabbard2pcMessage::VoteResponse(_, true) => {
                                (String::from(&message), Some("TRUE".to_string()))
                            }
                            Scabbard2pcMessage::VoteResponse(_, false) => {
                                (String::from(&message), Some("FALSE".to_string()))
                            }
                            _ => {
                                return Err(ScabbardStoreError::InvalidState(
                                    InvalidStateError::with_message(format!(
                                        "Failed to add consensus send message action, 
                                        invalid coordinator message type {}",
                                        String::from(&message)
                                    )),
                                ))
                            }
                        };

                        let send_message_action = Consensus2pcCoordinatorSendMessageActionModel {
                            action_id,
                            service_id: format!("{}", service_id),
                            epoch,
                            receiver_service_id: format!("{}", receiving_process),
                            message_type,
                            vote_response,
                        };
                        insert_into(consensus_2pc_coordinator_send_message_action::table)
                            .values(vec![send_message_action])
                            .execute(self.conn)?;
                        Ok(action_id)
                    }
                    ConsensusAction::Notify(notification) => {
                        let (notification_type, dropped_message) = match &notification {
                            ConsensusActionNotification::MessageDropped(message) => {
                                (String::from(&notification), Some(message.clone()))
                            }
                            ConsensusActionNotification::ParticipantRequestForVote(_) => {
                                return Err(ScabbardStoreError::InvalidState(
                                    InvalidStateError::with_message(format!(
                                        "Failed to add consensus notify action, 
                                            invalid coordinator notification type {}",
                                        String::from(&notification)
                                    )),
                                ))
                            }
                            _ => (String::from(&notification), None),
                        };

                        let notification_action = Consensus2pcCoordinatorNotificationModel {
                            action_id,
                            service_id: format!("{}", service_id),
                            epoch,
                            notification_type,
                            dropped_message,
                        };
                        insert_into(consensus_2pc_coordinator_notification_action::table)
                            .values(vec![notification_action])
                            .execute(self.conn)?;
                        Ok(action_id)
                    }
                }
            } else if participant_context.is_some() {
                let insertable_action = InsertableConsensus2pcActionModel {
                    service_id: format!("{}", service_id),
                    epoch,
                    executed_at: None,
                    position,
                };

                insert_into(consensus_2pc_action::table)
                    .values(vec![insertable_action])
                    .execute(self.conn)?;
                let action_id = consensus_2pc_action::table
                    .order(consensus_2pc_action::id.desc())
                    .select(consensus_2pc_action::id)
                    .first::<i64>(self.conn)?;

                match action {
                    ConsensusAction::Update(context, alarm) => match context {
                        ScabbardContext::Scabbard2pcContext(context) => {
                            let participant_action_alarm = get_timestamp(alarm)?;

                            let update_context_action =
                                Consensus2pcUpdateParticipantContextActionModel::try_from((
                                    &context,
                                    service_id,
                                    &action_id,
                                    &participant_action_alarm,
                                ))?;

                            insert_into(consensus_2pc_update_participant_context_action::table)
                                .values(vec![update_context_action])
                                .execute(self.conn)?;

                            let participants =
                                UpdateParticipantContextActionParticipantList::try_from((
                                    &context, service_id, &action_id,
                                ))?
                                .inner;
                            insert_into(
                                consensus_2pc_update_participant_context_action_participant::table,
                            )
                            .values(participants)
                            .execute(self.conn)?;
                            Ok(action_id)
                        }
                    },
                    ConsensusAction::SendMessage(receiving_process, message) => {
                        let (message_type, vote_request) = match &message {
                            Scabbard2pcMessage::DecisionRequest(_) => {
                                (String::from(&message), None)
                            }
                            Scabbard2pcMessage::Commit(_) => (String::from(&message), None),
                            Scabbard2pcMessage::Abort(_) => (String::from(&message), None),
                            Scabbard2pcMessage::VoteRequest(_, value) => {
                                (String::from(&message), Some(value.clone()))
                            }
                            _ => {
                                return Err(ScabbardStoreError::InvalidState(
                                    InvalidStateError::with_message(format!(
                                        "Failed to add consensus send message action, 
                                            invalid participant message type {}",
                                        String::from(&message)
                                    )),
                                ))
                            }
                        };

                        let send_message_action = Consensus2pcParticipantSendMessageActionModel {
                            action_id,
                            service_id: format!("{}", service_id),
                            epoch,
                            receiver_service_id: format!("{}", receiving_process),
                            message_type,
                            vote_request,
                        };
                        insert_into(consensus_2pc_participant_send_message_action::table)
                            .values(vec![send_message_action])
                            .execute(self.conn)?;
                        Ok(action_id)
                    }
                    ConsensusAction::Notify(notification) => {
                        let (notification_type, dropped_message, request_for_vote_value) =
                            match &notification {
                                ConsensusActionNotification::MessageDropped(message) => {
                                    (String::from(&notification), Some(message.clone()), None)
                                }
                                ConsensusActionNotification::ParticipantRequestForVote(value) => {
                                    (String::from(&notification), None, Some(value.clone()))
                                }
                                ConsensusActionNotification::RequestForStart()
                                | ConsensusActionNotification::CoordinatorRequestForVote() => {
                                    return Err(ScabbardStoreError::InvalidState(
                                        InvalidStateError::with_message(format!(
                                            "Failed to add consensus notify action, invalid 
                                            participant notification type {}",
                                            String::from(&notification)
                                        )),
                                    ))
                                }
                                _ => (String::from(&notification), None, None),
                            };

                        let notification_action = Consensus2pcParticipantNotificationModel {
                            action_id,
                            service_id: format!("{}", service_id),
                            epoch,
                            notification_type,
                            dropped_message,
                            request_for_vote_value,
                        };
                        insert_into(consensus_2pc_participant_notification_action::table)
                            .values(vec![notification_action])
                            .execute(self.conn)?;
                        Ok(action_id)
                    }
                }
            } else {
                Err(ScabbardStoreError::InvalidState(
                    InvalidStateError::with_message(format!(
                        "Faild to add consensus action, a context with service_id: {} and epoch: {} 
                        does not exist",
                        service_id, epoch
                    )),
                ))
            }
        })
    }
}

#[cfg(feature = "postgres")]
impl<'a> AddActionOperation for ScabbardStoreOperations<'a, PgConnection> {
    fn add_consensus_action(
        &self,
        action: ScabbardConsensusAction,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<i64, ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            let ScabbardConsensusAction::Scabbard2pcConsensusAction(action) = action;
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

            let position = consensus_2pc_action::table
                .filter(
                    consensus_2pc_action::service_id
                        .eq(format!("{}", service_id))
                        .and(consensus_2pc_action::epoch.eq(epoch)),
                )
                .order(consensus_2pc_action::position.desc())
                .select(consensus_2pc_action::position)
                .first::<i32>(self.conn)
                .optional()?
                .unwrap_or(0)
                + 1;

            if coordinator_context.is_some() {
                // return an error if there is both a coordinator and a participant context for the
                // given service_id and epoch
                if participant_context.is_some() {
                    return Err(ScabbardStoreError::InvalidState(
                        InvalidStateError::with_message(format!(
                            "Failed to add consensus action, contexts found for 
                                participant and coordinator with service_id: {} epoch: {} ",
                            service_id, epoch
                        )),
                    ));
                }

                let insertable_action = InsertableConsensus2pcActionModel {
                    service_id: format!("{}", service_id),
                    epoch,
                    executed_at: None,
                    position,
                };

                let action_id: i64 = insert_into(consensus_2pc_action::table)
                    .values(vec![insertable_action])
                    .returning(consensus_2pc_action::id)
                    .get_result(self.conn)?;

                match action {
                    ConsensusAction::Update(context, alarm) => match context {
                        ScabbardContext::Scabbard2pcContext(context) => {
                            let coordinator_action_alarm = get_timestamp(alarm)?;

                            let update_context_action =
                                Consensus2pcUpdateCoordinatorContextActionModel::try_from((
                                    &context,
                                    service_id,
                                    &action_id,
                                    &coordinator_action_alarm,
                                ))?;

                            insert_into(consensus_2pc_update_coordinator_context_action::table)
                                .values(vec![update_context_action])
                                .execute(self.conn)?;

                            let participants =
                                UpdateCoordinatorContextActionParticipantList::try_from((
                                    &context, service_id, &action_id,
                                ))?
                                .inner;
                            insert_into(
                                consensus_2pc_update_coordinator_context_action_participant::table,
                            )
                            .values(participants)
                            .execute(self.conn)?;

                            Ok(action_id)
                        }
                    },
                    ConsensusAction::SendMessage(receiving_process, message) => {
                        let (message_type, vote_response) = match message {
                            Scabbard2pcMessage::DecisionRequest(_) => {
                                (String::from(&message), None)
                            }
                            Scabbard2pcMessage::VoteResponse(_, true) => {
                                (String::from(&message), Some("TRUE".to_string()))
                            }
                            Scabbard2pcMessage::VoteResponse(_, false) => {
                                (String::from(&message), Some("FALSE".to_string()))
                            }
                            _ => {
                                return Err(ScabbardStoreError::InvalidState(
                                    InvalidStateError::with_message(format!(
                                        "Failed to add consensus send message action, 
                                        invalid coordinator message type {}",
                                        String::from(&message)
                                    )),
                                ))
                            }
                        };

                        let send_message_action = Consensus2pcCoordinatorSendMessageActionModel {
                            action_id,
                            service_id: format!("{}", service_id),
                            epoch,
                            receiver_service_id: format!("{}", receiving_process),
                            message_type,
                            vote_response,
                        };
                        insert_into(consensus_2pc_coordinator_send_message_action::table)
                            .values(vec![send_message_action])
                            .execute(self.conn)?;
                        Ok(action_id)
                    }
                    ConsensusAction::Notify(notification) => {
                        let (notification_type, dropped_message) = match &notification {
                            ConsensusActionNotification::MessageDropped(message) => {
                                (String::from(&notification), Some(message.clone()))
                            }
                            ConsensusActionNotification::ParticipantRequestForVote(_) => {
                                return Err(ScabbardStoreError::InvalidState(
                                    InvalidStateError::with_message(format!(
                                        "Failed to add consensus notify action, 
                                            invalid coordinator notification type {}",
                                        String::from(&notification)
                                    )),
                                ))
                            }
                            _ => (String::from(&notification), None),
                        };

                        let notification_action = Consensus2pcCoordinatorNotificationModel {
                            action_id,
                            service_id: format!("{}", service_id),
                            epoch,
                            notification_type,
                            dropped_message,
                        };
                        insert_into(consensus_2pc_coordinator_notification_action::table)
                            .values(vec![notification_action])
                            .execute(self.conn)?;
                        Ok(action_id)
                    }
                }
            } else if participant_context.is_some() {
                let insertable_action = InsertableConsensus2pcActionModel {
                    service_id: format!("{}", service_id),
                    epoch,
                    executed_at: None,
                    position,
                };

                let action_id: i64 = insert_into(consensus_2pc_action::table)
                    .values(vec![insertable_action])
                    .returning(consensus_2pc_action::id)
                    .get_result(self.conn)?;

                match action {
                    ConsensusAction::Update(context, alarm) => match context {
                        ScabbardContext::Scabbard2pcContext(context) => {
                            let participant_action_alarm = get_timestamp(alarm)?;

                            let update_context_action =
                                Consensus2pcUpdateParticipantContextActionModel::try_from((
                                    &context,
                                    service_id,
                                    &action_id,
                                    &participant_action_alarm,
                                ))?;

                            insert_into(consensus_2pc_update_participant_context_action::table)
                                .values(vec![update_context_action])
                                .execute(self.conn)?;

                            let participants =
                                UpdateParticipantContextActionParticipantList::try_from((
                                    &context, service_id, &action_id,
                                ))?
                                .inner;
                            insert_into(
                                consensus_2pc_update_participant_context_action_participant::table,
                            )
                            .values(participants)
                            .execute(self.conn)?;
                            Ok(action_id)
                        }
                    },
                    ConsensusAction::SendMessage(receiving_process, message) => {
                        let (message_type, vote_request) = match &message {
                            Scabbard2pcMessage::DecisionRequest(_) => {
                                (String::from(&message), None)
                            }
                            Scabbard2pcMessage::Commit(_) => (String::from(&message), None),
                            Scabbard2pcMessage::Abort(_) => (String::from(&message), None),
                            Scabbard2pcMessage::VoteRequest(_, value) => {
                                (String::from(&message), Some(value.clone()))
                            }
                            _ => {
                                return Err(ScabbardStoreError::InvalidState(
                                    InvalidStateError::with_message(format!(
                                        "Failed to add consensus send message action, 
                                            invalid participant message type {}",
                                        String::from(&message)
                                    )),
                                ))
                            }
                        };

                        let send_message_action = Consensus2pcParticipantSendMessageActionModel {
                            action_id,
                            service_id: format!("{}", service_id),
                            epoch,
                            receiver_service_id: format!("{}", receiving_process),
                            message_type,
                            vote_request,
                        };
                        insert_into(consensus_2pc_participant_send_message_action::table)
                            .values(vec![send_message_action])
                            .execute(self.conn)?;
                        Ok(action_id)
                    }
                    ConsensusAction::Notify(notification) => {
                        let (notification_type, dropped_message, request_for_vote_value) =
                            match &notification {
                                ConsensusActionNotification::MessageDropped(message) => {
                                    (String::from(&notification), Some(message.clone()), None)
                                }
                                ConsensusActionNotification::ParticipantRequestForVote(value) => {
                                    (String::from(&notification), None, Some(value.clone()))
                                }
                                ConsensusActionNotification::RequestForStart()
                                | ConsensusActionNotification::CoordinatorRequestForVote() => {
                                    return Err(ScabbardStoreError::InvalidState(
                                        InvalidStateError::with_message(format!(
                                            "Failed to add consensus notify action, invalid 
                                            participant notification type {}",
                                            String::from(&notification)
                                        )),
                                    ))
                                }
                                _ => (String::from(&notification), None, None),
                            };

                        let notification_action = Consensus2pcParticipantNotificationModel {
                            action_id,
                            service_id: format!("{}", service_id),
                            epoch,
                            notification_type,
                            dropped_message,
                            request_for_vote_value,
                        };
                        insert_into(consensus_2pc_participant_notification_action::table)
                            .values(vec![notification_action])
                            .execute(self.conn)?;
                        Ok(action_id)
                    }
                }
            } else {
                Err(ScabbardStoreError::InvalidState(
                    InvalidStateError::with_message(format!(
                        "Faild to add consensus action, a context with service_id: {} and epoch: {} 
                        does not exist",
                        service_id, epoch
                    )),
                ))
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
