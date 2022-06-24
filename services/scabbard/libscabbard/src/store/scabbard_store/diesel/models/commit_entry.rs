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
use splinter::service::FullyQualifiedServiceId;

use crate::store::scabbard_store::commit::{CommitEntry, CommitEntryBuilder, ConsensusDecision};

use crate::store::scabbard_store::diesel::schema::scabbard_v3_commit_history;

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
