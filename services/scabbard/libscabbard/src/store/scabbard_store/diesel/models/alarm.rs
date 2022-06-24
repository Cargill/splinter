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

use crate::store::scabbard_store::alarm::AlarmType;
use crate::store::scabbard_store::diesel::schema::scabbard_alarm;

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "scabbard_alarm"]
#[primary_key(circuit_id, service_id, alarm_type)]
pub struct ScabbardAlarmModel {
    pub circuit_id: String,
    pub service_id: String,
    pub alarm_type: AlarmTypeModel,
    pub alarm: i64, // timestamp, when to wake up
}

impl From<&AlarmType> for AlarmTypeModel {
    fn from(status: &AlarmType) -> Self {
        match *status {
            AlarmType::TwoPhaseCommit => AlarmTypeModel::TwoPhaseCommit,
        }
    }
}

impl From<&AlarmTypeModel> for AlarmType {
    fn from(status: &AlarmTypeModel) -> Self {
        match *status {
            AlarmTypeModel::TwoPhaseCommit => AlarmType::TwoPhaseCommit,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum AlarmTypeModel {
    TwoPhaseCommit,
}

// This has to be pub, due to its use in the table macro execution for IdentityModel
pub struct AlarmTypeModelMapping;

impl QueryId for AlarmTypeModelMapping {
    type QueryId = AlarmTypeModelMapping;
    const HAS_STATIC_QUERY_ID: bool = true;
}

impl NotNull for AlarmTypeModelMapping {}

impl SingleValue for AlarmTypeModelMapping {}

impl AsExpression<AlarmTypeModelMapping> for AlarmTypeModel {
    type Expression = Bound<AlarmTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl AsExpression<Nullable<AlarmTypeModelMapping>> for AlarmTypeModel {
    type Expression = Bound<Nullable<AlarmTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<AlarmTypeModelMapping> for &'a AlarmTypeModel {
    type Expression = Bound<AlarmTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<Nullable<AlarmTypeModelMapping>> for &'a AlarmTypeModel {
    type Expression = Bound<Nullable<AlarmTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<AlarmTypeModelMapping> for &'a &'b AlarmTypeModel {
    type Expression = Bound<AlarmTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<Nullable<AlarmTypeModelMapping>> for &'a &'b AlarmTypeModel {
    type Expression = Bound<Nullable<AlarmTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<DB: Backend> ToSql<AlarmTypeModelMapping, DB> for AlarmTypeModel {
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        match self {
            AlarmTypeModel::TwoPhaseCommit => out.write_all(b"TWOPHASECOMMIT")?,
        }
        Ok(IsNull::No)
    }
}

impl<DB> ToSql<Nullable<AlarmTypeModelMapping>, DB> for AlarmTypeModel
where
    DB: Backend,
    Self: ToSql<AlarmTypeModelMapping, DB>,
{
    fn to_sql<W: ::std::io::Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        ToSql::<AlarmTypeModelMapping, DB>::to_sql(self, out)
    }
}

impl<DB> Queryable<AlarmTypeModelMapping, DB> for AlarmTypeModel
where
    DB: Backend + HasSqlType<AlarmTypeModelMapping>,
    AlarmTypeModel: FromSql<AlarmTypeModelMapping, DB>,
{
    type Row = Self;

    fn build(row: Self::Row) -> Self {
        row
    }
}

impl<DB> FromSqlRow<AlarmTypeModelMapping, DB> for AlarmTypeModel
where
    DB: Backend,
    AlarmTypeModel: FromSql<AlarmTypeModelMapping, DB>,
{
    fn build_from_row<T: Row<DB>>(row: &mut T) -> deserialize::Result<Self> {
        FromSql::<AlarmTypeModelMapping, DB>::from_sql(row.take())
    }
}

#[cfg(feature = "postgres")]
impl FromSql<AlarmTypeModelMapping, Pg> for AlarmTypeModel {
    fn from_sql(bytes: Option<&<Pg as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes {
            Some(b"TWOPHASECOMMIT") => Ok(AlarmTypeModel::TwoPhaseCommit),
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
impl HasSqlType<AlarmTypeModelMapping> for Pg {
    fn metadata(lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        lookup.lookup_type("alarm_type")
    }
}

#[cfg(feature = "sqlite")]
impl FromSql<AlarmTypeModelMapping, Sqlite> for AlarmTypeModel {
    fn from_sql(bytes: Option<&<Sqlite as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes.map(|v| v.read_blob()) {
            Some(b"TWOPHASECOMMIT") => Ok(AlarmTypeModel::TwoPhaseCommit),
            Some(blob) => {
                Err(format!("Unexpected variant: {}", String::from_utf8_lossy(blob)).into())
            }
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

#[cfg(feature = "sqlite")]
impl HasSqlType<AlarmTypeModelMapping> for Sqlite {
    fn metadata(_lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        diesel::sqlite::SqliteType::Text
    }
}
