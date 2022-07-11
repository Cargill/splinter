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

use std::time::{Duration, SystemTime};

use diesel::prelude::*;
use diesel::sql_types::{BigInt, Binary, Nullable, Text, Timestamp};
use splinter::error::{InternalError, InvalidStateError};
use splinter::service::FullyQualifiedServiceId;

use crate::store::scabbard_store::diesel::models::{
    SupervisorNotificationModel, SupervisorNotificationTypeModel,
    SupervisorNotificationTypeModelMapping,
};
use crate::store::scabbard_store::diesel::schema::supervisor_notification;
use crate::store::scabbard_store::{
    identified::Identified, ScabbardStoreError, SupervisorNotification,
    SupervisorNotificationBuilder, SupervisorNotificationType,
};

use super::ScabbardStoreOperations;

const OPERATION_NAME: &str = "list_supervisor_notifications";

pub(in crate::store::scabbard_store::diesel) trait ListSupervisorNotificationOperation {
    fn list_supervisor_notifications(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Vec<Identified<SupervisorNotification>>, ScabbardStoreError>;
}

impl<'a, C> ListSupervisorNotificationOperation for ScabbardStoreOperations<'a, C>
where
    C: diesel::Connection,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
    <C as diesel::Connection>::Backend:
        diesel::types::HasSqlType<SupervisorNotificationTypeModelMapping>,
    SupervisorNotificationTypeModel:
        diesel::deserialize::FromSql<SupervisorNotificationTypeModelMapping, C::Backend>,
    SupervisorNotificationModel: diesel::Queryable<
        (
            BigInt,
            Text,
            Text,
            BigInt,
            SupervisorNotificationTypeModelMapping,
            Nullable<Binary>,
            Timestamp,
            Nullable<Timestamp>,
        ),
        C::Backend,
    >,
{
    fn list_supervisor_notifications(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Vec<Identified<SupervisorNotification>>, ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            let notification_models = supervisor_notification::table
                .filter(
                    supervisor_notification::service_id
                        .eq(service_id.service_id().as_str())
                        .and(
                            supervisor_notification::circuit_id
                                .eq(service_id.circuit_id().as_str()),
                        )
                        .and(supervisor_notification::executed_at.is_null()),
                )
                .order(supervisor_notification::id.asc())
                .load::<SupervisorNotificationModel>(self.conn)
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?;

            let notifications = notification_models
                .into_iter()
                .map(|notification| {
                    let created_at = SystemTime::UNIX_EPOCH
                        .checked_add(Duration::from_secs(
                            notification.created_at.timestamp() as u64
                        ))
                        .ok_or_else(|| {
                            InternalError::with_message(
                                "timestamp could not be represented as a `SystemTime`".to_string(),
                            )
                        })?;

                    let notification_type = match notification.notification_type {
                        SupervisorNotificationTypeModel::Abort => SupervisorNotificationType::Abort,
                        SupervisorNotificationTypeModel::Commit => {
                            SupervisorNotificationType::Commit
                        }
                        SupervisorNotificationTypeModel::RequestForStart => {
                            SupervisorNotificationType::RequestForStart
                        }
                        SupervisorNotificationTypeModel::CoordinatorRequestForVote => {
                            SupervisorNotificationType::CoordinatorRequestForVote
                        }
                        SupervisorNotificationTypeModel::ParticipantRequestForVote => {
                            if let Some(value) = notification.request_for_vote_value {
                                SupervisorNotificationType::ParticipantRequestForVote { value }
                            } else {
                                return Err(ScabbardStoreError::InvalidState(
                                    InvalidStateError::with_message(
                                        "ParticipantRequestForVote notficiation is missing value"
                                            .into(),
                                    ),
                                ));
                            }
                        }
                    };

                    let mut builder = SupervisorNotificationBuilder::default()
                        .with_service_id(service_id)
                        .with_action_id(notification.action_id)
                        .with_notification_type(&notification_type)
                        .with_created_at(created_at);

                    if let Some(executed_at) = notification.executed_at {
                        let executed_at = SystemTime::UNIX_EPOCH
                            .checked_add(Duration::from_secs(executed_at.timestamp() as u64))
                            .ok_or_else(|| {
                                InternalError::with_message(
                                    "timestamp could not be represented as a `SystemTime`"
                                        .to_string(),
                                )
                            })?;
                        builder = builder.with_executed_at(executed_at);
                    }

                    let record = builder
                        .build()
                        .map_err(|err| InternalError::from_source(Box::new(err)))?;

                    Ok(Identified {
                        id: notification.id,
                        record,
                    })
                })
                .collect::<Result<Vec<Identified<SupervisorNotification>>, ScabbardStoreError>>()?;

            Ok(notifications)
        })
    }
}
