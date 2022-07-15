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
use std::io::Write;

use splinter::error::InvalidArgumentError;
use splinter::service::{FullyQualifiedServiceId, ServiceId};

use super::{
    schema::{echo_peers, echo_request_errors, echo_requests, echo_services},
    EchoServiceStatus,
};
use crate::service::RequestStatus;

use crate::service::EchoRequest as ServiceEchoRequest;

use diesel::{
    backend::Backend,
    deserialize::{self, FromSql},
    expression::{helper_types::AsExprOf, AsExpression},
    serialize::{self, Output, ToSql},
    sql_types::SmallInt,
};

#[derive(Insertable, Queryable, Identifiable, PartialEq, Debug)]
#[table_name = "echo_services"]
#[primary_key(service_id)]
pub(crate) struct EchoService {
    pub service_id: String,
    pub frequency: Option<i64>,
    pub jitter: Option<i64>,
    pub error_rate: Option<f32>,
    pub status: EchoServiceStatusModel,
}

#[repr(i16)]
#[derive(Debug, Copy, Clone, PartialEq, FromSqlRow)]
pub enum EchoServiceStatusModel {
    Prepared = 1,
    Finalized = 2,
    Retired = 3,
}

impl From<EchoServiceStatusModel> for EchoServiceStatus {
    fn from(status: EchoServiceStatusModel) -> Self {
        match status {
            EchoServiceStatusModel::Prepared => EchoServiceStatus::Prepared,
            EchoServiceStatusModel::Finalized => EchoServiceStatus::Finalized,
            EchoServiceStatusModel::Retired => EchoServiceStatus::Retired,
        }
    }
}

impl<DB> ToSql<SmallInt, DB> for EchoServiceStatusModel
where
    DB: Backend,
    i16: ToSql<SmallInt, DB>,
{
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        (*self as i16).to_sql(out)
    }
}

impl AsExpression<SmallInt> for EchoServiceStatusModel {
    type Expression = AsExprOf<i16, SmallInt>;

    fn as_expression(self) -> Self::Expression {
        <i16 as AsExpression<SmallInt>>::as_expression(self as i16)
    }
}

impl<'a> AsExpression<SmallInt> for &'a EchoServiceStatusModel {
    type Expression = AsExprOf<i16, SmallInt>;

    fn as_expression(self) -> Self::Expression {
        <i16 as AsExpression<SmallInt>>::as_expression((*self) as i16)
    }
}

impl<DB> FromSql<SmallInt, DB> for EchoServiceStatusModel
where
    DB: Backend,
    i16: FromSql<SmallInt, DB>,
{
    fn from_sql(bytes: Option<&DB::RawValue>) -> deserialize::Result<Self> {
        match i16::from_sql(bytes)? {
            1 => Ok(EchoServiceStatusModel::Prepared),
            2 => Ok(EchoServiceStatusModel::Finalized),
            3 => Ok(EchoServiceStatusModel::Retired),
            int => Err(format!("Invalid echo service status {}", int).into()),
        }
    }
}

#[derive(Insertable, Queryable, Identifiable, PartialEq, Debug)]
#[table_name = "echo_peers"]
#[primary_key(service_id, peer_service_id)]
pub(crate) struct EchoPeer {
    pub service_id: String,
    pub peer_service_id: Option<String>,
}

#[derive(Insertable, Queryable, Identifiable, PartialEq, Debug)]
#[table_name = "echo_request_errors"]
#[primary_key(service_id, correlation_id)]
pub(crate) struct EchoRequestError {
    pub service_id: String,
    pub correlation_id: i64,
    pub error_message: String,
    pub error_at: i64,
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "echo_requests"]
#[primary_key(sender_service_id, correlation_id)]
pub(crate) struct EchoRequest {
    pub sender_service_id: String,
    pub correlation_id: i64,
    pub receiver_service_id: String,
    pub message: String,
    pub sent: Status,
    pub sent_at: Option<i64>,
    pub ack: Status,
    pub ack_at: Option<i64>,
}

impl TryFrom<EchoRequest> for ServiceEchoRequest {
    type Error = InvalidArgumentError;

    fn try_from(echo_request: EchoRequest) -> Result<Self, Self::Error> {
        let sender_service_id =
            FullyQualifiedServiceId::new_from_string(echo_request.sender_service_id)?;
        let receiver_service_id = ServiceId::new(echo_request.receiver_service_id)?;
        Ok(Self {
            sender_service_id,
            correlation_id: echo_request.correlation_id,
            receiver_service_id,
            message: echo_request.message,
            sent: RequestStatus::from(echo_request.sent),
            sent_at: echo_request.sent_at,
            ack: RequestStatus::from(echo_request.ack),
            ack_at: echo_request.ack_at,
        })
    }
}

#[repr(i16)]
#[derive(Debug, Copy, Clone, PartialEq, FromSqlRow)]
pub(crate) enum Status {
    NotSent = 0,
    Sent = 1,
}

impl From<Status> for RequestStatus {
    fn from(status: Status) -> Self {
        match status {
            Status::NotSent => RequestStatus::NotSent,
            Status::Sent => RequestStatus::Sent,
        }
    }
}

impl<DB> ToSql<SmallInt, DB> for Status
where
    DB: Backend,
    i16: ToSql<SmallInt, DB>,
{
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        (*self as i16).to_sql(out)
    }
}

impl AsExpression<SmallInt> for Status {
    type Expression = AsExprOf<i16, SmallInt>;

    fn as_expression(self) -> Self::Expression {
        <i16 as AsExpression<SmallInt>>::as_expression(self as i16)
    }
}

impl<'a> AsExpression<SmallInt> for &'a Status {
    type Expression = AsExprOf<i16, SmallInt>;

    fn as_expression(self) -> Self::Expression {
        <i16 as AsExpression<SmallInt>>::as_expression((*self) as i16)
    }
}

impl<DB> FromSql<SmallInt, DB> for Status
where
    DB: Backend,
    i16: FromSql<SmallInt, DB>,
{
    fn from_sql(bytes: Option<&DB::RawValue>) -> deserialize::Result<Self> {
        match i16::from_sql(bytes)? {
            0 => Ok(Status::NotSent),
            1 => Ok(Status::Sent),
            int => Err(format!("Invalid status {}", int).into()),
        }
    }
}
