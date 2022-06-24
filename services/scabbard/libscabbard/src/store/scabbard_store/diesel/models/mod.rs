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
use std::time::{Duration, SystemTime};

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

#[cfg(feature = "postgres")]
use diesel::pg::Pg;
#[cfg(feature = "sqlite")]
use diesel::sqlite::Sqlite;

use splinter::error::InternalError;
use splinter::service::{FullyQualifiedServiceId, ServiceId};

use crate::store::scabbard_store::{
    alarm::AlarmType,
    commit::{CommitEntry, CommitEntryBuilder, ConsensusDecision},
    service::{ConsensusType, ScabbardService, ServiceStatus},
    two_phase_commit::{Context, ContextBuilder, Event, Message, Notification, Participant, State},
    ConsensusContext,
};

use super::schema::{
    consensus_2pc_action, consensus_2pc_context, consensus_2pc_context_participant,
    consensus_2pc_deliver_event, consensus_2pc_event, consensus_2pc_notification_action,
    consensus_2pc_send_message_action, consensus_2pc_start_event,
    consensus_2pc_update_context_action, consensus_2pc_update_context_action_participant,
    consensus_2pc_vote_event, scabbard_alarm, scabbard_peer, scabbard_service,
    scabbard_v3_commit_history,
};

/// Database model representation of `ScabbardService`
#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "scabbard_service"]
#[primary_key(circuit_id, service_id)]
pub struct ScabbardServiceModel {
    pub circuit_id: String,
    pub service_id: String,
    pub consensus: ConsensusTypeModel,
    pub status: ServiceStatusTypeModel,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ConsensusTypeModel {
    Consensus2pc,
}

// This has to be pub, due to its use in the table macro execution for IdentityModel
pub struct ConsensusTypeModelMapping;

impl QueryId for ConsensusTypeModelMapping {
    type QueryId = ConsensusTypeModelMapping;
    const HAS_STATIC_QUERY_ID: bool = true;
}

impl NotNull for ConsensusTypeModelMapping {}

impl SingleValue for ConsensusTypeModelMapping {}

impl AsExpression<ConsensusTypeModelMapping> for ConsensusTypeModel {
    type Expression = Bound<ConsensusTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl AsExpression<Nullable<ConsensusTypeModelMapping>> for ConsensusTypeModel {
    type Expression = Bound<Nullable<ConsensusTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<ConsensusTypeModelMapping> for &'a ConsensusTypeModel {
    type Expression = Bound<ConsensusTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<Nullable<ConsensusTypeModelMapping>> for &'a ConsensusTypeModel {
    type Expression = Bound<Nullable<ConsensusTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<ConsensusTypeModelMapping> for &'a &'b ConsensusTypeModel {
    type Expression = Bound<ConsensusTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<Nullable<ConsensusTypeModelMapping>> for &'a &'b ConsensusTypeModel {
    type Expression = Bound<Nullable<ConsensusTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<DB: Backend> ToSql<ConsensusTypeModelMapping, DB> for ConsensusTypeModel {
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        match self {
            ConsensusTypeModel::Consensus2pc => out.write_all(b"2PC")?,
        }
        Ok(IsNull::No)
    }
}

impl<DB> ToSql<Nullable<ConsensusTypeModelMapping>, DB> for ConsensusTypeModel
where
    DB: Backend,
    Self: ToSql<ConsensusTypeModelMapping, DB>,
{
    fn to_sql<W: ::std::io::Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        ToSql::<ConsensusTypeModelMapping, DB>::to_sql(self, out)
    }
}

impl<DB> Queryable<ConsensusTypeModelMapping, DB> for ConsensusTypeModel
where
    DB: Backend + HasSqlType<ConsensusTypeModelMapping>,
    ConsensusTypeModel: FromSql<ConsensusTypeModelMapping, DB>,
{
    type Row = Self;

    fn build(row: Self::Row) -> Self {
        row
    }
}

impl<DB> FromSqlRow<ConsensusTypeModelMapping, DB> for ConsensusTypeModel
where
    DB: Backend,
    ConsensusTypeModel: FromSql<ConsensusTypeModelMapping, DB>,
{
    fn build_from_row<T: Row<DB>>(row: &mut T) -> deserialize::Result<Self> {
        FromSql::<ConsensusTypeModelMapping, DB>::from_sql(row.take())
    }
}

#[cfg(feature = "postgres")]
impl FromSql<ConsensusTypeModelMapping, Pg> for ConsensusTypeModel {
    fn from_sql(bytes: Option<&<Pg as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes {
            Some(b"2PC") => Ok(ConsensusTypeModel::Consensus2pc),
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
impl HasSqlType<ConsensusTypeModelMapping> for Pg {
    fn metadata(lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        lookup.lookup_type("scabbard_consensus")
    }
}

#[cfg(feature = "sqlite")]
impl FromSql<ConsensusTypeModelMapping, Sqlite> for ConsensusTypeModel {
    fn from_sql(bytes: Option<&<Sqlite as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes.map(|v| v.read_blob()) {
            Some(b"2PC") => Ok(ConsensusTypeModel::Consensus2pc),
            Some(blob) => {
                Err(format!("Unexpected variant: {}", String::from_utf8_lossy(blob)).into())
            }
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

#[cfg(feature = "sqlite")]
impl HasSqlType<ConsensusTypeModelMapping> for Sqlite {
    fn metadata(_lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        diesel::sqlite::SqliteType::Text
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ServiceStatusTypeModel {
    Prepared,
    Finalized,
    Retired,
}

// This has to be pub, due to its use in the table macro execution for IdentityModel
pub struct ServiceStatusTypeModelMapping;

impl QueryId for ServiceStatusTypeModelMapping {
    type QueryId = ServiceStatusTypeModelMapping;
    const HAS_STATIC_QUERY_ID: bool = true;
}

impl NotNull for ServiceStatusTypeModelMapping {}

impl SingleValue for ServiceStatusTypeModelMapping {}

impl AsExpression<ServiceStatusTypeModelMapping> for ServiceStatusTypeModel {
    type Expression = Bound<ServiceStatusTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl AsExpression<Nullable<ServiceStatusTypeModelMapping>> for ServiceStatusTypeModel {
    type Expression = Bound<Nullable<ServiceStatusTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<ServiceStatusTypeModelMapping> for &'a ServiceStatusTypeModel {
    type Expression = Bound<ServiceStatusTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<Nullable<ServiceStatusTypeModelMapping>> for &'a ServiceStatusTypeModel {
    type Expression = Bound<Nullable<ServiceStatusTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<ServiceStatusTypeModelMapping> for &'a &'b ServiceStatusTypeModel {
    type Expression = Bound<ServiceStatusTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<Nullable<ServiceStatusTypeModelMapping>>
    for &'a &'b ServiceStatusTypeModel
{
    type Expression = Bound<Nullable<ServiceStatusTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<DB: Backend> ToSql<ServiceStatusTypeModelMapping, DB> for ServiceStatusTypeModel {
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        match self {
            ServiceStatusTypeModel::Prepared => out.write_all(b"PREPARED")?,
            ServiceStatusTypeModel::Finalized => out.write_all(b"FINALIZED")?,
            ServiceStatusTypeModel::Retired => out.write_all(b"RETIRED")?,
        }
        Ok(IsNull::No)
    }
}

impl<DB> ToSql<Nullable<ServiceStatusTypeModelMapping>, DB> for ServiceStatusTypeModel
where
    DB: Backend,
    Self: ToSql<ServiceStatusTypeModelMapping, DB>,
{
    fn to_sql<W: ::std::io::Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        ToSql::<ServiceStatusTypeModelMapping, DB>::to_sql(self, out)
    }
}

impl<DB> Queryable<ServiceStatusTypeModelMapping, DB> for ServiceStatusTypeModel
where
    DB: Backend + HasSqlType<ServiceStatusTypeModelMapping>,
    ServiceStatusTypeModel: FromSql<ServiceStatusTypeModelMapping, DB>,
{
    type Row = Self;

    fn build(row: Self::Row) -> Self {
        row
    }
}

impl<DB> FromSqlRow<ServiceStatusTypeModelMapping, DB> for ServiceStatusTypeModel
where
    DB: Backend,
    ServiceStatusTypeModel: FromSql<ServiceStatusTypeModelMapping, DB>,
{
    fn build_from_row<T: Row<DB>>(row: &mut T) -> deserialize::Result<Self> {
        FromSql::<ServiceStatusTypeModelMapping, DB>::from_sql(row.take())
    }
}

#[cfg(feature = "postgres")]
impl FromSql<ServiceStatusTypeModelMapping, Pg> for ServiceStatusTypeModel {
    fn from_sql(bytes: Option<&<Pg as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes {
            Some(b"PREPARED") => Ok(ServiceStatusTypeModel::Prepared),
            Some(b"FINALIZED") => Ok(ServiceStatusTypeModel::Finalized),
            Some(b"RETIRED") => Ok(ServiceStatusTypeModel::Retired),
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
impl HasSqlType<ServiceStatusTypeModelMapping> for Pg {
    fn metadata(lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        lookup.lookup_type("scabbard_service_status_type")
    }
}

#[cfg(feature = "sqlite")]
impl FromSql<ServiceStatusTypeModelMapping, Sqlite> for ServiceStatusTypeModel {
    fn from_sql(bytes: Option<&<Sqlite as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes.map(|v| v.read_blob()) {
            Some(b"PREPARED") => Ok(ServiceStatusTypeModel::Prepared),
            Some(b"FINALIZED") => Ok(ServiceStatusTypeModel::Finalized),
            Some(b"RETIRED") => Ok(ServiceStatusTypeModel::Retired),
            Some(blob) => {
                Err(format!("Unexpected variant: {}", String::from_utf8_lossy(blob)).into())
            }
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

#[cfg(feature = "sqlite")]
impl HasSqlType<ServiceStatusTypeModelMapping> for Sqlite {
    fn metadata(_lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        diesel::sqlite::SqliteType::Text
    }
}

impl From<&ScabbardService> for ScabbardServiceModel {
    fn from(service: &ScabbardService) -> Self {
        ScabbardServiceModel {
            circuit_id: service.service_id().circuit_id().to_string(),
            service_id: service.service_id().service_id().to_string(),
            consensus: service.consensus().into(),
            status: service.status().into(),
        }
    }
}

/// Database model representation of `ScabbardService` peer
#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "scabbard_peer"]
#[primary_key(circuit_id, service_id, peer_service_id)]
pub struct ScabbardPeerModel {
    pub circuit_id: String,
    pub service_id: String,
    pub peer_service_id: String,
}

impl From<&ScabbardService> for Vec<ScabbardPeerModel> {
    fn from(service: &ScabbardService) -> Self {
        service
            .peers()
            .iter()
            .map(|service_id| ScabbardPeerModel {
                circuit_id: service.service_id().circuit_id().to_string(),
                service_id: service.service_id().service_id().to_string(),
                peer_service_id: service_id.to_string(),
            })
            .collect::<Vec<ScabbardPeerModel>>()
    }
}

/// Database model representation of `ScabbardService` commit entry
#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "scabbard_v3_commit_history"]
#[primary_key(circuit_id, service_id, epoch)]
pub struct CommitEntryModel {
    pub circuit_id: String,
    pub service_id: String,
    pub epoch: i64,
    pub value: String,
    pub decision: Option<DecisionTypeModel>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum DecisionTypeModel {
    Commit,
    Abort,
}

// This has to be pub, due to its use in the table macro execution for Decision
pub struct DecisionTypeModelMapping;

impl QueryId for DecisionTypeModelMapping {
    type QueryId = DecisionTypeModelMapping;
    const HAS_STATIC_QUERY_ID: bool = true;
}

impl NotNull for DecisionTypeModelMapping {}

impl SingleValue for DecisionTypeModelMapping {}

impl AsExpression<DecisionTypeModelMapping> for DecisionTypeModel {
    type Expression = Bound<DecisionTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl AsExpression<Nullable<DecisionTypeModelMapping>> for DecisionTypeModel {
    type Expression = Bound<Nullable<DecisionTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<DecisionTypeModelMapping> for &'a DecisionTypeModel {
    type Expression = Bound<DecisionTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<Nullable<DecisionTypeModelMapping>> for &'a DecisionTypeModel {
    type Expression = Bound<Nullable<DecisionTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<DecisionTypeModelMapping> for &'a &'b DecisionTypeModel {
    type Expression = Bound<DecisionTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<Nullable<DecisionTypeModelMapping>> for &'a &'b DecisionTypeModel {
    type Expression = Bound<Nullable<DecisionTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<DB: Backend> ToSql<DecisionTypeModelMapping, DB> for DecisionTypeModel {
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        match self {
            DecisionTypeModel::Commit => out.write_all(b"COMMIT")?,
            DecisionTypeModel::Abort => out.write_all(b"ABORT")?,
        }
        Ok(IsNull::No)
    }
}

impl<DB> ToSql<Nullable<DecisionTypeModelMapping>, DB> for DecisionTypeModel
where
    DB: Backend,
    Self: ToSql<DecisionTypeModelMapping, DB>,
{
    fn to_sql<W: ::std::io::Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        ToSql::<DecisionTypeModelMapping, DB>::to_sql(self, out)
    }
}

impl<DB> Queryable<DecisionTypeModelMapping, DB> for DecisionTypeModel
where
    DB: Backend + HasSqlType<DecisionTypeModelMapping>,
    DecisionTypeModel: FromSql<DecisionTypeModelMapping, DB>,
{
    type Row = Self;

    fn build(row: Self::Row) -> Self {
        row
    }
}

impl<DB> FromSqlRow<DecisionTypeModelMapping, DB> for DecisionTypeModel
where
    DB: Backend,
    DecisionTypeModel: FromSql<DecisionTypeModelMapping, DB>,
{
    fn build_from_row<T: Row<DB>>(row: &mut T) -> deserialize::Result<Self> {
        FromSql::<DecisionTypeModelMapping, DB>::from_sql(row.take())
    }
}

#[cfg(feature = "postgres")]
impl FromSql<DecisionTypeModelMapping, Pg> for DecisionTypeModel {
    fn from_sql(bytes: Option<&<Pg as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes {
            Some(b"COMMIT") => Ok(DecisionTypeModel::Commit),
            Some(b"ABORT") => Ok(DecisionTypeModel::Abort),
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
impl HasSqlType<DecisionTypeModelMapping> for Pg {
    fn metadata(lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        lookup.lookup_type("decision_type")
    }
}

#[cfg(feature = "sqlite")]
impl FromSql<DecisionTypeModelMapping, Sqlite> for DecisionTypeModel {
    fn from_sql(bytes: Option<&<Sqlite as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes.map(|v| v.read_blob()) {
            Some(b"COMMIT") => Ok(DecisionTypeModel::Commit),
            Some(b"ABORT") => Ok(DecisionTypeModel::Abort),
            Some(blob) => {
                Err(format!("Unexpected variant: {}", String::from_utf8_lossy(blob)).into())
            }
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

#[cfg(feature = "sqlite")]
impl HasSqlType<DecisionTypeModelMapping> for Sqlite {
    fn metadata(_lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        diesel::sqlite::SqliteType::Text
    }
}

impl TryFrom<&CommitEntry> for CommitEntryModel {
    type Error = InternalError;

    fn try_from(entry: &CommitEntry) -> Result<Self, Self::Error> {
        Ok(CommitEntryModel {
            circuit_id: entry.service_id().circuit_id().to_string(),
            service_id: entry.service_id().service_id().to_string(),
            epoch: i64::try_from(entry.epoch().ok_or_else(|| {
                InternalError::with_message("Epoch is not set on commit entry".to_string())
            })?)
            .map_err(|err| InternalError::from_source(Box::new(err)))?,
            value: entry.value().to_string(),
            decision: entry
                .decision()
                .clone()
                .map(|decision| DecisionTypeModel::from(&decision)),
        })
    }
}

impl TryFrom<CommitEntryModel> for CommitEntry {
    type Error = InternalError;

    fn try_from(entry: CommitEntryModel) -> Result<Self, Self::Error> {
        let service_id = FullyQualifiedServiceId::new_from_string(format!(
            "{}::{}",
            entry.circuit_id, entry.service_id
        ))
        .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let mut builder = CommitEntryBuilder::default()
            .with_service_id(&service_id)
            .with_epoch(
                u64::try_from(entry.epoch)
                    .map_err(|err| InternalError::from_source(Box::new(err)))?,
            )
            .with_value(&entry.value);

        if let Some(d) = entry.decision {
            builder = builder.with_decision(&ConsensusDecision::from(&d));
        }

        builder
            .build()
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }
}

impl TryFrom<&str> for ServiceStatus {
    type Error = InternalError;

    fn try_from(status: &str) -> Result<Self, Self::Error> {
        match status {
            "PREPARED" => Ok(ServiceStatus::Prepared),
            "FINALIZED" => Ok(ServiceStatus::Finalized),
            "RETIRED" => Ok(ServiceStatus::Retired),
            _ => Err(InternalError::with_message(format!(
                "Unknown status {}",
                status
            ))),
        }
    }
}

impl From<&ServiceStatus> for String {
    fn from(status: &ServiceStatus) -> Self {
        match *status {
            ServiceStatus::Prepared => "PREPARED".into(),
            ServiceStatus::Finalized => "FINALIZED".into(),
            ServiceStatus::Retired => "RETIRED".into(),
        }
    }
}

impl From<&ServiceStatusTypeModel> for ServiceStatus {
    fn from(status: &ServiceStatusTypeModel) -> Self {
        match status {
            ServiceStatusTypeModel::Prepared => ServiceStatus::Prepared,
            ServiceStatusTypeModel::Finalized => ServiceStatus::Finalized,
            ServiceStatusTypeModel::Retired => ServiceStatus::Retired,
        }
    }
}

impl From<&ServiceStatus> for ServiceStatusTypeModel {
    fn from(status: &ServiceStatus) -> Self {
        match *status {
            ServiceStatus::Prepared => ServiceStatusTypeModel::Prepared,
            ServiceStatus::Finalized => ServiceStatusTypeModel::Finalized,
            ServiceStatus::Retired => ServiceStatusTypeModel::Retired,
        }
    }
}

impl TryFrom<&str> for ConsensusType {
    type Error = InternalError;

    fn try_from(consensus: &str) -> Result<Self, Self::Error> {
        match consensus {
            "2PC" => Ok(ConsensusType::TwoPC),
            _ => Err(InternalError::with_message(format!(
                "Unknown consensus {}",
                consensus
            ))),
        }
    }
}

impl From<&ConsensusType> for String {
    fn from(consensus: &ConsensusType) -> Self {
        match *consensus {
            ConsensusType::TwoPC => "2PC".into(),
        }
    }
}

impl From<&ConsensusTypeModel> for ConsensusType {
    fn from(consensus: &ConsensusTypeModel) -> Self {
        match consensus {
            ConsensusTypeModel::Consensus2pc => ConsensusType::TwoPC,
        }
    }
}

impl From<&ConsensusType> for ConsensusTypeModel {
    fn from(consensus: &ConsensusType) -> Self {
        match *consensus {
            ConsensusType::TwoPC => ConsensusTypeModel::Consensus2pc,
        }
    }
}

impl TryFrom<&str> for ConsensusDecision {
    type Error = InternalError;

    fn try_from(status: &str) -> Result<Self, Self::Error> {
        match status {
            "ABORT" => Ok(ConsensusDecision::Abort),
            "COMMIT" => Ok(ConsensusDecision::Commit),
            _ => Err(InternalError::with_message(format!(
                "Unknown decision {}",
                status
            ))),
        }
    }
}

impl From<&ConsensusDecision> for String {
    fn from(status: &ConsensusDecision) -> Self {
        match *status {
            ConsensusDecision::Abort => "ABORT".into(),
            ConsensusDecision::Commit => "COMMIT".into(),
        }
    }
}

impl From<&DecisionTypeModel> for ConsensusDecision {
    fn from(status: &DecisionTypeModel) -> Self {
        match status {
            DecisionTypeModel::Abort => ConsensusDecision::Abort,
            DecisionTypeModel::Commit => ConsensusDecision::Commit,
        }
    }
}

impl From<&ConsensusDecision> for DecisionTypeModel {
    fn from(status: &ConsensusDecision) -> Self {
        match *status {
            ConsensusDecision::Abort => DecisionTypeModel::Abort,
            ConsensusDecision::Commit => DecisionTypeModel::Commit,
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "scabbard_alarm"]
#[primary_key(circuit_id, service_id, alarm_type)]
pub struct ScabbardAlarmModel {
    pub circuit_id: String,
    pub service_id: String,
    pub alarm_type: String,
    pub alarm: i64, // timestamp, when to wake up
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_context"]
#[primary_key(circuit_id, service_id)]
pub struct Consensus2pcContextModel {
    pub circuit_id: String,
    pub service_id: String,
    pub coordinator: String,
    pub epoch: i64,
    pub last_commit_epoch: Option<i64>,
    pub state: ContextStateModel,
    pub vote_timeout_start: Option<i64>,
    pub vote: Option<String>,
    pub decision_timeout_start: Option<i64>,
    pub ack_timeout_start: Option<i64>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ContextStateModel {
    Abort,
    Commit,
    Voted,
    Voting,
    WaitingForStart,
    WaitingForVoteRequest,
    WaitingForVote,
    WaitingForDecisionAck,
}

pub struct ContextStateModelMapping;

impl QueryId for ContextStateModelMapping {
    type QueryId = ContextStateModelMapping;
    const HAS_STATIC_QUERY_ID: bool = true;
}

impl NotNull for ContextStateModelMapping {}

impl SingleValue for ContextStateModelMapping {}

impl AsExpression<ContextStateModelMapping> for ContextStateModel {
    type Expression = Bound<ContextStateModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl AsExpression<Nullable<ContextStateModelMapping>> for ContextStateModel {
    type Expression = Bound<Nullable<ContextStateModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<ContextStateModelMapping> for &'a ContextStateModel {
    type Expression = Bound<ContextStateModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<Nullable<ContextStateModelMapping>> for &'a ContextStateModel {
    type Expression = Bound<Nullable<ContextStateModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<ContextStateModelMapping> for &'a &'b ContextStateModel {
    type Expression = Bound<ContextStateModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<Nullable<ContextStateModelMapping>> for &'a &'b ContextStateModel {
    type Expression = Bound<Nullable<ContextStateModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<DB: Backend> ToSql<ContextStateModelMapping, DB> for ContextStateModel {
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        match self {
            ContextStateModel::Abort => out.write_all(b"ABORT")?,
            ContextStateModel::Commit => out.write_all(b"COMMIT")?,
            ContextStateModel::Voted => out.write_all(b"VOTED")?,
            ContextStateModel::Voting => out.write_all(b"VOTING")?,
            ContextStateModel::WaitingForStart => out.write_all(b"WAITINGFORSTART")?,
            ContextStateModel::WaitingForVoteRequest => out.write_all(b"WAITINGFORVOTEREQUEST")?,
            ContextStateModel::WaitingForVote => out.write_all(b"WAITINGFORVOTE")?,
            ContextStateModel::WaitingForDecisionAck => {
                out.write_all(b"WAITING_FOR_DECISION_ACK")?
            }
        }
        Ok(IsNull::No)
    }
}

impl<DB> ToSql<Nullable<ContextStateModelMapping>, DB> for ContextStateModel
where
    DB: Backend,
    Self: ToSql<ContextStateModelMapping, DB>,
{
    fn to_sql<W: ::std::io::Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        ToSql::<ContextStateModelMapping, DB>::to_sql(self, out)
    }
}

impl<DB> Queryable<ContextStateModelMapping, DB> for ContextStateModel
where
    DB: Backend + HasSqlType<ContextStateModelMapping>,
    ContextStateModel: FromSql<ContextStateModelMapping, DB>,
{
    type Row = Self;

    fn build(row: Self::Row) -> Self {
        row
    }
}

impl<DB> FromSqlRow<ContextStateModelMapping, DB> for ContextStateModel
where
    DB: Backend,
    ContextStateModel: FromSql<ContextStateModelMapping, DB>,
{
    fn build_from_row<T: Row<DB>>(row: &mut T) -> deserialize::Result<Self> {
        FromSql::<ContextStateModelMapping, DB>::from_sql(row.take())
    }
}

#[cfg(feature = "postgres")]
impl FromSql<ContextStateModelMapping, Pg> for ContextStateModel {
    fn from_sql(bytes: Option<&<Pg as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes {
            Some(b"ABORT") => Ok(ContextStateModel::Abort),
            Some(b"COMMIT") => Ok(ContextStateModel::Commit),
            Some(b"VOTED") => Ok(ContextStateModel::Voted),
            Some(b"VOTING") => Ok(ContextStateModel::Voting),
            Some(b"WAITINGFORSTART") => Ok(ContextStateModel::WaitingForStart),
            Some(b"WAITINGFORVOTEREQUEST") => Ok(ContextStateModel::WaitingForVoteRequest),
            Some(b"WAITINGFORVOTE") => Ok(ContextStateModel::WaitingForVote),
            Some(b"WAITING_FOR_DECISION_ACK") => Ok(ContextStateModel::WaitingForDecisionAck),
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
impl HasSqlType<ContextStateModelMapping> for Pg {
    fn metadata(lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        lookup.lookup_type("context_state")
    }
}

#[cfg(feature = "sqlite")]
impl FromSql<ContextStateModelMapping, Sqlite> for ContextStateModel {
    fn from_sql(bytes: Option<&<Sqlite as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes.map(|v| v.read_blob()) {
            Some(b"ABORT") => Ok(ContextStateModel::Abort),
            Some(b"COMMIT") => Ok(ContextStateModel::Commit),
            Some(b"VOTED") => Ok(ContextStateModel::Voted),
            Some(b"VOTING") => Ok(ContextStateModel::Voting),
            Some(b"WAITINGFORSTART") => Ok(ContextStateModel::WaitingForStart),
            Some(b"WAITINGFORVOTEREQUEST") => Ok(ContextStateModel::WaitingForVoteRequest),
            Some(b"WAITINGFORVOTE") => Ok(ContextStateModel::WaitingForVote),
            Some(b"WAITING_FOR_DECISION_ACK") => Ok(ContextStateModel::WaitingForDecisionAck),
            Some(blob) => {
                Err(format!("Unexpected variant: {}", String::from_utf8_lossy(blob)).into())
            }
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

#[cfg(feature = "sqlite")]
impl HasSqlType<ContextStateModelMapping> for Sqlite {
    fn metadata(_lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        diesel::sqlite::SqliteType::Text
    }
}

impl
    TryFrom<(
        &Consensus2pcContextModel,
        Vec<Consensus2pcContextParticipantModel>,
    )> for ConsensusContext
{
    type Error = InternalError;

    fn try_from(
        (context, participants): (
            &Consensus2pcContextModel,
            Vec<Consensus2pcContextParticipantModel>,
        ),
    ) -> Result<Self, Self::Error> {
        let epoch = u64::try_from(context.epoch)
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        let coordinator = ServiceId::new(&context.coordinator)
            .map_err(|e| InternalError::from_source(Box::new(e)))?;
        let last_commit_epoch = context
            .last_commit_epoch
            .map(u64::try_from)
            .transpose()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        let participants = ParticipantList::try_from(participants)?.inner;

        let state = match context.state {
            ContextStateModel::WaitingForStart => State::WaitingForStart,
            ContextStateModel::Voting => {
                let vote_timeout_start = if let Some(t) = context.vote_timeout_start {
                    SystemTime::UNIX_EPOCH
                        .checked_add(Duration::from_secs(t as u64))
                        .ok_or_else(|| {
                            InternalError::with_message(
                                "Failed to convert vote timeout start timestamp to SystemTime"
                                    .to_string(),
                            )
                        })?
                } else {
                    return Err(InternalError::with_message(
                        "Failed to convert to ConsensusContext, context has state 'voting' but \
                        no vote timeout start time set"
                            .to_string(),
                    ));
                };
                State::Voting { vote_timeout_start }
            }
            ContextStateModel::WaitingForVote => State::WaitingForVote,
            ContextStateModel::Abort => State::Abort,
            ContextStateModel::Commit => State::Commit,
            ContextStateModel::WaitingForVoteRequest => State::WaitingForVoteRequest,
            ContextStateModel::Voted => {
                let vote = context
                    .vote
                    .as_ref()
                    .map(|v| match v.as_str() {
                        "TRUE" => Some(true),
                        "FALSE" => Some(false),
                        _ => None,
                    })
                    .ok_or_else(|| {
                        InternalError::with_message(
                        "Failed to convert to ConsensusContext, context has state 'voted' but vote \
                        is unset"
                        .to_string(),
                    )
                    })?
                    .ok_or_else(|| {
                        InternalError::with_message(
                            "Failed to convert to ConsensusContext, context has state 'voted' but \
                        an invalid vote response was found"
                                .to_string(),
                        )
                    })?;
                let decision_timeout_start = if let Some(t) = context.decision_timeout_start {
                    SystemTime::UNIX_EPOCH
                        .checked_add(Duration::from_secs(t as u64))
                        .ok_or_else(|| {
                            InternalError::with_message(
                                "Failed to convert decision timeout start timestamp to SystemTime"
                                    .to_string(),
                            )
                        })?
                } else {
                    return Err(InternalError::with_message(
                        "Failed to convert to ConsensusContext, context has state 'voted' but \
                        'decision_timeout_start' is unset"
                            .to_string(),
                    ));
                };
                State::Voted {
                    vote,
                    decision_timeout_start,
                }
            }
            ContextStateModel::WaitingForDecisionAck => {
                let ack_timeout_start = if let Some(t) = context.ack_timeout_start {
                    SystemTime::UNIX_EPOCH
                        .checked_add(Duration::from_secs(t as u64))
                        .ok_or_else(|| {
                            InternalError::with_message(
                                "Failed to convert ack timeout start timestamp to SystemTime"
                                    .to_string(),
                            )
                        })?
                } else {
                    return Err(InternalError::with_message(
                        "Failed to convert to ConsensusContext, context has state \
                        'WaitingForDecisionAck' but 'decision_timeout_start' is unset"
                            .to_string(),
                    ));
                };
                State::WaitingForDecisionAck { ack_timeout_start }
            }
        };

        let mut builder = ContextBuilder::default()
            .with_coordinator(&coordinator)
            .with_epoch(epoch)
            .with_state(state)
            .with_participants(participants)
            .with_this_process(
                FullyQualifiedServiceId::new_from_string(format!(
                    "{}::{}",
                    &context.circuit_id, &context.service_id
                ))
                .map_err(|err| InternalError::from_source(Box::new(err)))?
                .service_id(),
            );

        if let Some(last_commit_epoch) = last_commit_epoch {
            builder = builder.with_last_commit_epoch(last_commit_epoch);
        }
        let context = builder
            .build()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;
        Ok(ConsensusContext::TwoPhaseCommit(context))
    }
}

impl TryFrom<(&Context, &FullyQualifiedServiceId)> for Consensus2pcContextModel {
    type Error = InternalError;

    fn try_from(
        (context, service_id): (&Context, &FullyQualifiedServiceId),
    ) -> Result<Self, Self::Error> {
        let epoch = i64::try_from(*context.epoch())
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        let last_commit_epoch = context
            .last_commit_epoch()
            .map(i64::try_from)
            .transpose()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        let (vote_timeout_start, vote, decision_timeout_start, ack_timeout_start) =
            match context.state() {
                State::Voting { vote_timeout_start } => {
                    let time = i64::try_from(
                        vote_timeout_start
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .map_err(|err| InternalError::from_source(Box::new(err)))?
                            .as_secs(),
                    )
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                    (Some(time), None, None, None)
                }
                State::Voted {
                    vote,
                    decision_timeout_start,
                } => {
                    let time = i64::try_from(
                        decision_timeout_start
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .map_err(|err| InternalError::from_source(Box::new(err)))?
                            .as_secs(),
                    )
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                    let vote = match vote {
                        true => "TRUE",
                        false => "FALSE",
                    };
                    (None, Some(vote.to_string()), Some(time), None)
                }
                State::WaitingForDecisionAck { ack_timeout_start } => {
                    let time = i64::try_from(
                        ack_timeout_start
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .map_err(|err| InternalError::from_source(Box::new(err)))?
                            .as_secs(),
                    )
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                    (None, None, None, Some(time))
                }
                _ => (None, None, None, None),
            };
        let state = ContextStateModel::from(context.state());
        Ok(Consensus2pcContextModel {
            circuit_id: service_id.circuit_id().to_string(),
            service_id: service_id.service_id().to_string(),
            coordinator: format!("{}", context.coordinator()),
            epoch,
            last_commit_epoch,
            state,
            vote_timeout_start,
            vote,
            decision_timeout_start,
            ack_timeout_start,
        })
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_context_participant"]
#[primary_key(circuit_id, service_id, process)]
pub struct Consensus2pcContextParticipantModel {
    pub circuit_id: String,
    pub service_id: String,
    pub epoch: i64,
    pub process: String,
    pub vote: Option<String>,
    pub decision_ack: bool,
}

#[derive(Debug, PartialEq)]
pub struct ParticipantList {
    pub inner: Vec<Participant>,
}

impl TryFrom<Vec<Consensus2pcContextParticipantModel>> for ParticipantList {
    type Error = InternalError;

    fn try_from(
        participants: Vec<Consensus2pcContextParticipantModel>,
    ) -> Result<Self, Self::Error> {
        let mut all_participants = Vec::new();
        for p in participants {
            let vote = if let Some(vote) = p.vote {
                match vote.as_str() {
                    "TRUE" => Some(true),
                    "FALSE" => Some(false),
                    _ => {
                        return Err(InternalError::with_message(format!(
                        "Failed to convert context participant model to participant, invalid vote \
                        value found: {}",
                        vote,
                    )))
                    }
                }
            } else {
                None
            };
            all_participants.push(Participant {
                process: ServiceId::new(p.process)
                    .map_err(|e| InternalError::from_source(Box::new(e)))?,
                vote,
                decision_ack: p.decision_ack,
            });
        }
        Ok(ParticipantList {
            inner: all_participants,
        })
    }
}

#[derive(Debug, PartialEq)]
pub struct ContextParticipantList {
    pub inner: Vec<Consensus2pcContextParticipantModel>,
}

impl TryFrom<(&Context, &FullyQualifiedServiceId)> for ContextParticipantList {
    type Error = InternalError;

    fn try_from(
        (context, service_id): (&Context, &FullyQualifiedServiceId),
    ) -> Result<Self, Self::Error> {
        let epoch = i64::try_from(*context.epoch())
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        let mut participants = Vec::new();
        for participant in context.participants() {
            let vote = participant.vote.map(|vote| match vote {
                true => "TRUE".to_string(),
                false => "FALSE".to_string(),
            });
            participants.push(Consensus2pcContextParticipantModel {
                circuit_id: service_id.circuit_id().to_string(),
                service_id: service_id.service_id().to_string(),
                epoch,
                process: format!("{}", participant.process),
                vote,
                decision_ack: participant.decision_ack,
            })
        }
        Ok(ContextParticipantList {
            inner: participants,
        })
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_update_context_action"]
#[belongs_to(Consensus2pcActionModel, foreign_key = "action_id")]
#[primary_key(action_id)]
pub struct Consensus2pcUpdateContextActionModel {
    pub action_id: i64,
    pub coordinator: String,
    pub epoch: i64,
    pub last_commit_epoch: Option<i64>,
    pub state: ContextStateModel,
    pub vote_timeout_start: Option<i64>,
    pub vote: Option<String>,
    pub decision_timeout_start: Option<i64>,
    pub action_alarm: Option<i64>,
    pub ack_timeout_start: Option<i64>,
}

impl TryFrom<(&Context, &i64, &Option<i64>)> for Consensus2pcUpdateContextActionModel {
    type Error = InternalError;

    fn try_from(
        (context, action_id, action_alarm): (&Context, &i64, &Option<i64>),
    ) -> Result<Self, Self::Error> {
        let epoch = i64::try_from(*context.epoch())
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        let last_commit_epoch = context
            .last_commit_epoch()
            .map(i64::try_from)
            .transpose()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        let (vote_timeout_start, vote, decision_timeout_start, ack_timeout_start) =
            match context.state() {
                State::Voting { vote_timeout_start } => {
                    let time = i64::try_from(
                        vote_timeout_start
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .map_err(|err| InternalError::from_source(Box::new(err)))?
                            .as_secs(),
                    )
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                    (Some(time), None, None, None)
                }
                State::Voted {
                    vote,
                    decision_timeout_start,
                } => {
                    let time = i64::try_from(
                        decision_timeout_start
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .map_err(|err| InternalError::from_source(Box::new(err)))?
                            .as_secs(),
                    )
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                    let vote = match vote {
                        true => "TRUE",
                        false => "FALSE",
                    };
                    (None, Some(vote.to_string()), Some(time), None)
                }
                State::WaitingForDecisionAck { ack_timeout_start } => {
                    let time = i64::try_from(
                        ack_timeout_start
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .map_err(|err| InternalError::from_source(Box::new(err)))?
                            .as_secs(),
                    )
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                    (None, None, None, Some(time))
                }
                _ => (None, None, None, None),
            };
        let state = ContextStateModel::from(context.state());
        Ok(Consensus2pcUpdateContextActionModel {
            action_id: *action_id,
            coordinator: format!("{}", context.coordinator()),
            epoch,
            last_commit_epoch,
            state,
            vote_timeout_start,
            vote,
            decision_timeout_start,
            action_alarm: *action_alarm,
            ack_timeout_start,
        })
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_update_context_action_participant"]
#[belongs_to(Consensus2pcActionModel, foreign_key = "action_id")]
#[belongs_to(Consensus2pcUpdateContextActionModel, foreign_key = "action_id")]
#[primary_key(action_id, process)]
pub struct Consensus2pcUpdateContextActionParticipantModel {
    pub action_id: i64,
    pub process: String,
    pub vote: Option<String>,
    pub decision_ack: bool,
}

#[derive(Debug, PartialEq)]
pub struct UpdateContextActionParticipantList {
    pub inner: Vec<Consensus2pcUpdateContextActionParticipantModel>,
}

impl TryFrom<(&Context, &i64)> for UpdateContextActionParticipantList {
    type Error = InternalError;

    fn try_from((context, action_id): (&Context, &i64)) -> Result<Self, Self::Error> {
        let mut participants = Vec::new();
        for participant in context.participants() {
            let vote = participant.vote.map(|vote| match vote {
                true => "TRUE".to_string(),
                false => "FALSE".to_string(),
            });
            participants.push(Consensus2pcUpdateContextActionParticipantModel {
                action_id: *action_id,
                process: format!("{}", participant.process),
                vote,
                decision_ack: participant.decision_ack,
            })
        }
        Ok(UpdateContextActionParticipantList {
            inner: participants,
        })
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_send_message_action"]
#[belongs_to(Consensus2pcActionModel, foreign_key = "action_id")]
#[primary_key(action_id)]
pub struct Consensus2pcSendMessageActionModel {
    pub action_id: i64,
    pub epoch: i64,
    pub receiver_service_id: String,
    pub message_type: MessageTypeModel,
    pub vote_response: Option<String>,
    pub vote_request: Option<Vec<u8>>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum MessageTypeModel {
    VoteResponse,
    DecisionRequest,
    VoteRequest,
    Commit,
    Abort,
    DecisionAck,
}

pub struct MessageTypeModelMapping;

impl QueryId for MessageTypeModelMapping {
    type QueryId = MessageTypeModelMapping;
    const HAS_STATIC_QUERY_ID: bool = true;
}

impl NotNull for MessageTypeModelMapping {}

impl SingleValue for MessageTypeModelMapping {}

impl AsExpression<MessageTypeModelMapping> for MessageTypeModel {
    type Expression = Bound<MessageTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl AsExpression<Nullable<MessageTypeModelMapping>> for MessageTypeModel {
    type Expression = Bound<Nullable<MessageTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<MessageTypeModelMapping> for &'a MessageTypeModel {
    type Expression = Bound<MessageTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<Nullable<MessageTypeModelMapping>> for &'a MessageTypeModel {
    type Expression = Bound<Nullable<MessageTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<MessageTypeModelMapping> for &'a &'b MessageTypeModel {
    type Expression = Bound<MessageTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<Nullable<MessageTypeModelMapping>> for &'a &'b MessageTypeModel {
    type Expression = Bound<Nullable<MessageTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<DB: Backend> ToSql<MessageTypeModelMapping, DB> for MessageTypeModel {
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        match self {
            MessageTypeModel::VoteResponse => out.write_all(b"VOTERESPONSE")?,
            MessageTypeModel::DecisionRequest => out.write_all(b"DECISIONREQUEST")?,
            MessageTypeModel::VoteRequest => out.write_all(b"VOTEREQUEST")?,
            MessageTypeModel::Commit => out.write_all(b"COMMIT")?,
            MessageTypeModel::Abort => out.write_all(b"ABORT")?,
            MessageTypeModel::DecisionAck => out.write_all(b"DECISIONACK")?,
        }
        Ok(IsNull::No)
    }
}

impl<DB> ToSql<Nullable<MessageTypeModelMapping>, DB> for MessageTypeModel
where
    DB: Backend,
    Self: ToSql<MessageTypeModelMapping, DB>,
{
    fn to_sql<W: ::std::io::Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        ToSql::<MessageTypeModelMapping, DB>::to_sql(self, out)
    }
}

impl<DB> Queryable<MessageTypeModelMapping, DB> for MessageTypeModel
where
    DB: Backend + HasSqlType<MessageTypeModelMapping>,
    MessageTypeModel: FromSql<MessageTypeModelMapping, DB>,
{
    type Row = Self;

    fn build(row: Self::Row) -> Self {
        row
    }
}

impl<DB> FromSqlRow<MessageTypeModelMapping, DB> for MessageTypeModel
where
    DB: Backend,
    MessageTypeModel: FromSql<MessageTypeModelMapping, DB>,
{
    fn build_from_row<T: Row<DB>>(row: &mut T) -> deserialize::Result<Self> {
        FromSql::<MessageTypeModelMapping, DB>::from_sql(row.take())
    }
}

#[cfg(feature = "postgres")]
impl FromSql<MessageTypeModelMapping, Pg> for MessageTypeModel {
    fn from_sql(bytes: Option<&<Pg as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes {
            Some(b"VOTERESPONSE") => Ok(MessageTypeModel::VoteResponse),
            Some(b"DECISIONREQUEST") => Ok(MessageTypeModel::DecisionRequest),
            Some(b"VOTEREQUEST") => Ok(MessageTypeModel::VoteRequest),
            Some(b"COMMIT") => Ok(MessageTypeModel::Commit),
            Some(b"ABORT") => Ok(MessageTypeModel::Abort),
            Some(b"DECISIONACK") => Ok(MessageTypeModel::DecisionAck),
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
impl HasSqlType<MessageTypeModelMapping> for Pg {
    fn metadata(lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        lookup.lookup_type("message_type")
    }
}

#[cfg(feature = "sqlite")]
impl FromSql<MessageTypeModelMapping, Sqlite> for MessageTypeModel {
    fn from_sql(bytes: Option<&<Sqlite as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes.map(|v| v.read_blob()) {
            Some(b"VOTERESPONSE") => Ok(MessageTypeModel::VoteResponse),
            Some(b"DECISIONREQUEST") => Ok(MessageTypeModel::DecisionRequest),
            Some(b"VOTEREQUEST") => Ok(MessageTypeModel::VoteRequest),
            Some(b"COMMIT") => Ok(MessageTypeModel::Commit),
            Some(b"ABORT") => Ok(MessageTypeModel::Abort),
            Some(b"DECISIONACK") => Ok(MessageTypeModel::DecisionAck),
            Some(blob) => {
                Err(format!("Unexpected variant: {}", String::from_utf8_lossy(blob)).into())
            }
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

#[cfg(feature = "sqlite")]
impl HasSqlType<MessageTypeModelMapping> for Sqlite {
    fn metadata(_lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        diesel::sqlite::SqliteType::Text
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_notification_action"]
#[belongs_to(Consensus2pcActionModel, foreign_key = "action_id")]
#[primary_key(action_id)]
pub struct Consensus2pcNotificationModel {
    pub action_id: i64,
    pub notification_type: NotificationTypeModel,
    pub dropped_message: Option<String>,
    pub request_for_vote_value: Option<Vec<u8>>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum NotificationTypeModel {
    RequestForStart,
    CoordinatorRequestForVote,
    ParticipantRequestForVote,
    Commit,
    Abort,
    MessageDropped,
}

pub struct NotificationTypeModelMapping;

impl QueryId for NotificationTypeModelMapping {
    type QueryId = NotificationTypeModelMapping;
    const HAS_STATIC_QUERY_ID: bool = true;
}

impl NotNull for NotificationTypeModelMapping {}

impl SingleValue for NotificationTypeModelMapping {}

impl AsExpression<NotificationTypeModelMapping> for NotificationTypeModel {
    type Expression = Bound<NotificationTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl AsExpression<Nullable<NotificationTypeModelMapping>> for NotificationTypeModel {
    type Expression = Bound<Nullable<NotificationTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<NotificationTypeModelMapping> for &'a NotificationTypeModel {
    type Expression = Bound<NotificationTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<Nullable<NotificationTypeModelMapping>> for &'a NotificationTypeModel {
    type Expression = Bound<Nullable<NotificationTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<NotificationTypeModelMapping> for &'a &'b NotificationTypeModel {
    type Expression = Bound<NotificationTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<Nullable<NotificationTypeModelMapping>>
    for &'a &'b NotificationTypeModel
{
    type Expression = Bound<Nullable<NotificationTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<DB: Backend> ToSql<NotificationTypeModelMapping, DB> for NotificationTypeModel {
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        match self {
            NotificationTypeModel::RequestForStart => out.write_all(b"REQUESTFORSTART")?,
            NotificationTypeModel::CoordinatorRequestForVote => {
                out.write_all(b"COORDINATORREQUESTFORVOTE")?
            }
            NotificationTypeModel::ParticipantRequestForVote => {
                out.write_all(b"PARTICIPANTREQUESTFORVOTE")?
            }
            NotificationTypeModel::Commit => out.write_all(b"COMMIT")?,
            NotificationTypeModel::Abort => out.write_all(b"ABORT")?,
            NotificationTypeModel::MessageDropped => out.write_all(b"MESSAGEDROPPED")?,
        }
        Ok(IsNull::No)
    }
}

impl<DB> ToSql<Nullable<NotificationTypeModelMapping>, DB> for NotificationTypeModel
where
    DB: Backend,
    Self: ToSql<NotificationTypeModelMapping, DB>,
{
    fn to_sql<W: ::std::io::Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        ToSql::<NotificationTypeModelMapping, DB>::to_sql(self, out)
    }
}

impl<DB> Queryable<NotificationTypeModelMapping, DB> for NotificationTypeModel
where
    DB: Backend + HasSqlType<NotificationTypeModelMapping>,
    NotificationTypeModel: FromSql<NotificationTypeModelMapping, DB>,
{
    type Row = Self;

    fn build(row: Self::Row) -> Self {
        row
    }
}

impl<DB> FromSqlRow<NotificationTypeModelMapping, DB> for NotificationTypeModel
where
    DB: Backend,
    NotificationTypeModel: FromSql<NotificationTypeModelMapping, DB>,
{
    fn build_from_row<T: Row<DB>>(row: &mut T) -> deserialize::Result<Self> {
        FromSql::<NotificationTypeModelMapping, DB>::from_sql(row.take())
    }
}

#[cfg(feature = "postgres")]
impl FromSql<NotificationTypeModelMapping, Pg> for NotificationTypeModel {
    fn from_sql(bytes: Option<&<Pg as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes {
            Some(b"REQUESTFORSTART") => Ok(NotificationTypeModel::RequestForStart),
            Some(b"COORDINATORREQUESTFORVOTE") => {
                Ok(NotificationTypeModel::CoordinatorRequestForVote)
            }
            Some(b"PARTICIPANTREQUESTFORVOTE") => {
                Ok(NotificationTypeModel::ParticipantRequestForVote)
            }
            Some(b"COMMIT") => Ok(NotificationTypeModel::Commit),
            Some(b"ABORT") => Ok(NotificationTypeModel::Abort),
            Some(b"MESSAGEDROPPED") => Ok(NotificationTypeModel::MessageDropped),
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
impl HasSqlType<NotificationTypeModelMapping> for Pg {
    fn metadata(lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        lookup.lookup_type("notification_type")
    }
}

#[cfg(feature = "sqlite")]
impl FromSql<NotificationTypeModelMapping, Sqlite> for NotificationTypeModel {
    fn from_sql(bytes: Option<&<Sqlite as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes.map(|v| v.read_blob()) {
            Some(b"REQUESTFORSTART") => Ok(NotificationTypeModel::RequestForStart),
            Some(b"COORDINATORREQUESTFORVOTE") => {
                Ok(NotificationTypeModel::CoordinatorRequestForVote)
            }
            Some(b"PARTICIPANTREQUESTFORVOTE") => {
                Ok(NotificationTypeModel::ParticipantRequestForVote)
            }
            Some(b"COMMIT") => Ok(NotificationTypeModel::Commit),
            Some(b"ABORT") => Ok(NotificationTypeModel::Abort),
            Some(b"MESSAGEDROPPED") => Ok(NotificationTypeModel::MessageDropped),
            Some(blob) => {
                Err(format!("Unexpected variant: {}", String::from_utf8_lossy(blob)).into())
            }
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

#[cfg(feature = "sqlite")]
impl HasSqlType<NotificationTypeModelMapping> for Sqlite {
    fn metadata(_lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        diesel::sqlite::SqliteType::Text
    }
}

impl From<&State> for String {
    fn from(state: &State) -> Self {
        match *state {
            State::WaitingForStart => String::from("WAITINGFORSTART"),
            State::Voting { .. } => String::from("VOTING"),
            State::WaitingForVote => String::from("WAITINGFORVOTE"),
            State::Abort => String::from("ABORT"),
            State::Commit => String::from("COMMIT"),
            State::WaitingForVoteRequest => String::from("WAITINGFORVOTEREQUEST"),
            State::Voted { .. } => String::from("VOTED"),
            State::WaitingForDecisionAck { .. } => String::from("WAITING_FOR_DECISION_ACK"),
        }
    }
}

impl From<&State> for ContextStateModel {
    fn from(state: &State) -> Self {
        match *state {
            State::WaitingForStart => ContextStateModel::WaitingForStart,
            State::Voting { .. } => ContextStateModel::Voting,
            State::WaitingForVote => ContextStateModel::WaitingForVote,
            State::Abort => ContextStateModel::Abort,
            State::Commit => ContextStateModel::Commit,
            State::WaitingForVoteRequest => ContextStateModel::WaitingForVoteRequest,
            State::Voted { .. } => ContextStateModel::Voted,
            State::WaitingForDecisionAck { .. } => ContextStateModel::WaitingForDecisionAck,
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_action"]
#[belongs_to(Consensus2pcContextModel, foreign_key = "service_id")]
#[primary_key(id)]
pub struct Consensus2pcActionModel {
    pub id: i64,
    pub circuit_id: String,
    pub service_id: String,
    pub created_at: SystemTime,
    pub executed_at: Option<i64>,
}

#[derive(Debug, PartialEq, Insertable)]
#[table_name = "consensus_2pc_action"]
pub struct InsertableConsensus2pcActionModel {
    pub circuit_id: String,
    pub service_id: String,
    pub executed_at: Option<i64>,
}

impl From<&Notification> for String {
    fn from(notification: &Notification) -> Self {
        match *notification {
            Notification::RequestForStart() => String::from("REQUESTFORSTART"),
            Notification::CoordinatorRequestForVote() => String::from("COORDINATORREQUESTFORVOTE"),
            Notification::ParticipantRequestForVote(_) => String::from("PARTICIPANTREQUESTFORVOTE"),
            Notification::Commit() => String::from("COMMIT"),
            Notification::Abort() => String::from("ABORT"),
            Notification::MessageDropped(_) => String::from("MESSAGEDROPPED"),
        }
    }
}

impl From<&Notification> for NotificationTypeModel {
    fn from(notification: &Notification) -> Self {
        match *notification {
            Notification::RequestForStart() => NotificationTypeModel::RequestForStart,
            Notification::CoordinatorRequestForVote() => {
                NotificationTypeModel::CoordinatorRequestForVote
            }
            Notification::ParticipantRequestForVote(_) => {
                NotificationTypeModel::ParticipantRequestForVote
            }
            Notification::Commit() => NotificationTypeModel::Commit,
            Notification::Abort() => NotificationTypeModel::Abort,
            Notification::MessageDropped(_) => NotificationTypeModel::MessageDropped,
        }
    }
}

impl From<&Message> for String {
    fn from(message: &Message) -> Self {
        match *message {
            Message::VoteRequest(..) => String::from("VOTEREQUEST"),
            Message::DecisionRequest(_) => String::from("DECISIONREQUEST"),
            Message::VoteResponse(..) => String::from("VOTERESPONSE"),
            Message::Commit(_) => String::from("COMMIT"),
            Message::Abort(_) => String::from("ABORT"),
            Message::DecisionAck(_) => String::from("DECISION_ACK"),
        }
    }
}

impl From<&Message> for MessageTypeModel {
    fn from(message: &Message) -> Self {
        match *message {
            Message::VoteRequest(..) => MessageTypeModel::VoteRequest,
            Message::DecisionRequest(_) => MessageTypeModel::DecisionRequest,
            Message::VoteResponse(..) => MessageTypeModel::VoteResponse,
            Message::Commit(_) => MessageTypeModel::Commit,
            Message::Abort(_) => MessageTypeModel::Abort,
            Message::DecisionAck(_) => MessageTypeModel::DecisionAck,
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_event"]
#[primary_key(id)]
pub struct Consensus2pcEventModel {
    pub id: i64,
    pub circuit_id: String,
    pub service_id: String,
    pub created_at: SystemTime,
    pub executed_at: Option<i64>,
    pub event_type: String,
}

#[derive(Debug, PartialEq, Insertable)]
#[table_name = "consensus_2pc_event"]
pub struct InsertableConsensus2pcEventModel {
    pub circuit_id: String,
    pub service_id: String,
    pub executed_at: Option<i64>,
    pub event_type: String,
}

impl From<&Event> for String {
    fn from(event: &Event) -> Self {
        match *event {
            Event::Alarm() => "ALARM".into(),
            Event::Deliver(..) => "DELIVER".into(),
            Event::Start(..) => "START".into(),
            Event::Vote(..) => "VOTE".into(),
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_deliver_event"]
#[belongs_to(Consensus2pcEventModel, foreign_key = "event_id")]
#[primary_key(event_id)]
pub struct Consensus2pcDeliverEventModel {
    pub event_id: i64,
    pub epoch: i64,
    pub receiver_service_id: String,
    pub message_type: MessageTypeModel,
    pub vote_response: Option<String>,
    pub vote_request: Option<Vec<u8>>,
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_start_event"]
#[belongs_to(Consensus2pcEventModel, foreign_key = "event_id")]
#[primary_key(event_id)]
pub struct Consensus2pcStartEventModel {
    pub event_id: i64,
    pub value: Vec<u8>,
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_vote_event"]
#[belongs_to(Consensus2pcEventModel, foreign_key = "event_id")]
#[primary_key(event_id)]
pub struct Consensus2pcVoteEventModel {
    pub event_id: i64,
    pub vote: String, // TRUE or FALSE
}

impl From<&AlarmType> for String {
    fn from(status: &AlarmType) -> Self {
        match *status {
            AlarmType::TwoPhaseCommit => "TWOPHASECOMMIT".into(),
        }
    }
}
