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

use crate::store::scabbard_store::diesel::schema::{scabbard_peer, scabbard_service};
use crate::store::scabbard_store::service::{ConsensusType, ScabbardService, ServiceStatus};

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
