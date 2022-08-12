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

use std::io::Write;

use chrono::naive::NaiveDateTime;
#[cfg(feature = "postgres")]
use diesel::pg::Pg;
#[cfg(feature = "sqlite")]
use diesel::sqlite::Sqlite;
use diesel::{
    backend::Backend,
    deserialize::FromSqlRow,
    expression::{bound::Bound, AsExpression},
    query_builder::QueryId,
    serialize::{self, IsNull, Output, ToSql},
    sql_types::{HasSqlType, NotNull, Nullable, SingleValue},
    Queryable,
};
#[cfg(feature = "diesel")]
use diesel::{
    deserialize::{self, FromSql},
    row::Row,
};
use splinter::error::InternalError;

use crate::store::scabbard_store::{SupervisorNotification, SupervisorNotificationType};

use crate::store::scabbard_store::diesel::schema::supervisor_notification;

#[derive(
    Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, QueryableByName,
)]
#[table_name = "supervisor_notification"]
#[primary_key(id)]
pub struct SupervisorNotificationModel {
    pub id: i64,
    pub circuit_id: String,
    pub service_id: String,
    pub action_id: i64,
    pub notification_type: SupervisorNotificationTypeModel,
    pub request_for_vote_value: Option<Vec<u8>>,
    pub created_at: NaiveDateTime,
    pub executed_at: Option<NaiveDateTime>,
}

#[derive(Debug, PartialEq, Eq, Insertable)]
#[table_name = "supervisor_notification"]
pub struct InsertableSupervisorNotificationModel {
    pub circuit_id: String,
    pub service_id: String,
    pub action_id: i64,
    pub notification_type: SupervisorNotificationTypeModel,
    pub request_for_vote_value: Option<Vec<u8>>,
    pub executed_at: Option<NaiveDateTime>,
}

impl TryFrom<&SupervisorNotification> for InsertableSupervisorNotificationModel {
    type Error = InternalError;

    fn try_from(notification: &SupervisorNotification) -> Result<Self, Self::Error> {
        let executed_at = match notification.executed_at() {
            Some(time) => {
                let duration = time
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                let seconds = i64::try_from(duration.as_secs())
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                Some(NaiveDateTime::from_timestamp(
                    seconds,
                    duration.subsec_nanos(),
                ))
            }
            None => None,
        };

        let request_for_vote_value = match notification.notification_type() {
            SupervisorNotificationType::ParticipantRequestForVote { value } => Some(value.to_vec()),
            _ => None,
        };

        Ok(InsertableSupervisorNotificationModel {
            circuit_id: notification.service_id().circuit_id().to_string(),
            service_id: notification.service_id().service_id().to_string(),
            action_id: *notification.action_id(),
            notification_type: notification.notification_type().into(),
            request_for_vote_value,
            executed_at,
        })
    }
}

impl From<&SupervisorNotificationType> for String {
    fn from(notification_type: &SupervisorNotificationType) -> Self {
        match notification_type {
            SupervisorNotificationType::Abort => "ABORT".to_string(),
            SupervisorNotificationType::Commit => "COMMIT".to_string(),
            SupervisorNotificationType::RequestForStart => "REQUEST_FOR_START".to_string(),
            SupervisorNotificationType::CoordinatorRequestForVote => {
                "COORDINATOR_REQUEST_FOR_VOTE".to_string()
            }
            SupervisorNotificationType::ParticipantRequestForVote { .. } => {
                "PARTICIPANT_REQUEST_FOR_VOTE".to_string()
            }
        }
    }
}

impl From<&SupervisorNotificationType> for SupervisorNotificationTypeModel {
    fn from(notification_type: &SupervisorNotificationType) -> Self {
        match notification_type {
            SupervisorNotificationType::Abort => SupervisorNotificationTypeModel::Abort,
            SupervisorNotificationType::Commit => SupervisorNotificationTypeModel::Commit,
            SupervisorNotificationType::RequestForStart => {
                SupervisorNotificationTypeModel::RequestForStart
            }
            SupervisorNotificationType::CoordinatorRequestForVote => {
                SupervisorNotificationTypeModel::CoordinatorRequestForVote
            }
            SupervisorNotificationType::ParticipantRequestForVote { .. } => {
                SupervisorNotificationTypeModel::ParticipantRequestForVote
            }
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SupervisorNotificationTypeModel {
    RequestForStart,
    CoordinatorRequestForVote,
    ParticipantRequestForVote,
    Commit,
    Abort,
}

pub struct SupervisorNotificationTypeModelMapping;

impl QueryId for SupervisorNotificationTypeModelMapping {
    type QueryId = SupervisorNotificationTypeModelMapping;
    const HAS_STATIC_QUERY_ID: bool = true;
}

impl NotNull for SupervisorNotificationTypeModelMapping {}

impl SingleValue for SupervisorNotificationTypeModelMapping {}

impl AsExpression<SupervisorNotificationTypeModelMapping> for SupervisorNotificationTypeModel {
    type Expression = Bound<SupervisorNotificationTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl AsExpression<Nullable<SupervisorNotificationTypeModelMapping>>
    for SupervisorNotificationTypeModel
{
    type Expression = Bound<Nullable<SupervisorNotificationTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<SupervisorNotificationTypeModelMapping>
    for &'a SupervisorNotificationTypeModel
{
    type Expression = Bound<SupervisorNotificationTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<Nullable<SupervisorNotificationTypeModelMapping>>
    for &'a SupervisorNotificationTypeModel
{
    type Expression = Bound<Nullable<SupervisorNotificationTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<SupervisorNotificationTypeModelMapping>
    for &'a &'b SupervisorNotificationTypeModel
{
    type Expression = Bound<SupervisorNotificationTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<Nullable<SupervisorNotificationTypeModelMapping>>
    for &'a &'b SupervisorNotificationTypeModel
{
    type Expression = Bound<Nullable<SupervisorNotificationTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<DB: Backend> ToSql<SupervisorNotificationTypeModelMapping, DB>
    for SupervisorNotificationTypeModel
{
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        match self {
            SupervisorNotificationTypeModel::RequestForStart => {
                out.write_all(b"REQUEST_FOR_START")?
            }
            SupervisorNotificationTypeModel::CoordinatorRequestForVote => {
                out.write_all(b"COORDINATOR_REQUEST_FOR_VOTE")?
            }
            SupervisorNotificationTypeModel::ParticipantRequestForVote => {
                out.write_all(b"PARTICIPANT_REQUEST_FOR_VOTE")?
            }
            SupervisorNotificationTypeModel::Commit => out.write_all(b"COMMIT")?,
            SupervisorNotificationTypeModel::Abort => out.write_all(b"ABORT")?,
        }
        Ok(IsNull::No)
    }
}

impl<DB> ToSql<Nullable<SupervisorNotificationTypeModelMapping>, DB>
    for SupervisorNotificationTypeModel
where
    DB: Backend,
    Self: ToSql<SupervisorNotificationTypeModelMapping, DB>,
{
    fn to_sql<W: ::std::io::Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        ToSql::<SupervisorNotificationTypeModelMapping, DB>::to_sql(self, out)
    }
}

impl<DB> Queryable<SupervisorNotificationTypeModelMapping, DB> for SupervisorNotificationTypeModel
where
    DB: Backend + HasSqlType<SupervisorNotificationTypeModelMapping>,
    SupervisorNotificationTypeModel: FromSql<SupervisorNotificationTypeModelMapping, DB>,
{
    type Row = Self;

    fn build(row: Self::Row) -> Self {
        row
    }
}

impl<DB> FromSqlRow<SupervisorNotificationTypeModelMapping, DB> for SupervisorNotificationTypeModel
where
    DB: Backend,
    SupervisorNotificationTypeModel: FromSql<SupervisorNotificationTypeModelMapping, DB>,
{
    fn build_from_row<T: Row<DB>>(row: &mut T) -> deserialize::Result<Self> {
        FromSql::<SupervisorNotificationTypeModelMapping, DB>::from_sql(row.take())
    }
}

#[cfg(feature = "postgres")]
impl FromSql<SupervisorNotificationTypeModelMapping, Pg> for SupervisorNotificationTypeModel {
    fn from_sql(bytes: Option<&<Pg as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes {
            Some(b"REQUEST_FOR_START") => Ok(SupervisorNotificationTypeModel::RequestForStart),
            Some(b"COORDINATOR_REQUEST_FOR_VOTE") => {
                Ok(SupervisorNotificationTypeModel::CoordinatorRequestForVote)
            }
            Some(b"PARTICIPANT_REQUEST_FOR_VOTE") => {
                Ok(SupervisorNotificationTypeModel::ParticipantRequestForVote)
            }
            Some(b"COMMIT") => Ok(SupervisorNotificationTypeModel::Commit),
            Some(b"ABORT") => Ok(SupervisorNotificationTypeModel::Abort),
            Some(v) => Err(format!(
                "Unrecognized enum variant: '{}'",
                String::from_utf8_lossy(v)
            )
            .into()),
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

#[cfg(feature = "postgres")]
impl HasSqlType<SupervisorNotificationTypeModelMapping> for Pg {
    fn metadata(lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        lookup.lookup_type("supervisor_notification_type")
    }
}

#[cfg(feature = "sqlite")]
impl FromSql<SupervisorNotificationTypeModelMapping, Sqlite> for SupervisorNotificationTypeModel {
    fn from_sql(bytes: Option<&<Sqlite as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes.map(|v| v.read_blob()) {
            Some(b"REQUEST_FOR_START") => Ok(SupervisorNotificationTypeModel::RequestForStart),
            Some(b"COORDINATOR_REQUEST_FOR_VOTE") => {
                Ok(SupervisorNotificationTypeModel::CoordinatorRequestForVote)
            }
            Some(b"PARTICIPANT_REQUEST_FOR_VOTE") => {
                Ok(SupervisorNotificationTypeModel::ParticipantRequestForVote)
            }
            Some(b"COMMIT") => Ok(SupervisorNotificationTypeModel::Commit),
            Some(b"ABORT") => Ok(SupervisorNotificationTypeModel::Abort),
            Some(blob) => {
                Err(format!("Unexpected variant: {}", String::from_utf8_lossy(blob)).into())
            }
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

#[cfg(feature = "sqlite")]
impl HasSqlType<SupervisorNotificationTypeModelMapping> for Sqlite {
    fn metadata(_lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        diesel::sqlite::SqliteType::Text
    }
}
