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

#[cfg(any(feature = "postgres", feature = "sqlite"))]
use diesel::{
    deserialize::{self, FromSql},
    row::Row,
};

#[cfg(feature = "postgres")]
use diesel::pg::Pg;
#[cfg(feature = "sqlite")]
use diesel::sqlite::Sqlite;

use super::schema::{rbac_assignments, rbac_identities, rbac_role_permissions, rbac_roles};

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable)]
#[table_name = "rbac_roles"]
#[primary_key(id)]
pub(super) struct RoleModel {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable)]
#[table_name = "rbac_role_permissions"]
#[belongs_to(RoleModel, foreign_key = "role_id")]
#[primary_key(role_id, permission)]
pub(super) struct RolePermissionModel {
    pub role_id: String,
    pub permission: String,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub(super) enum IdentityModelType {
    Key,
    User,
}

// This has to be pub, due to its use in the table macro execution for IdentityModel
pub struct IdentityModelTypeMapping;

impl QueryId for IdentityModelTypeMapping {
    type QueryId = IdentityModelTypeMapping;
    const HAS_STATIC_QUERY_ID: bool = true;
}

impl NotNull for IdentityModelTypeMapping {}

impl SingleValue for IdentityModelTypeMapping {}

impl AsExpression<IdentityModelTypeMapping> for IdentityModelType {
    type Expression = Bound<IdentityModelTypeMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl AsExpression<Nullable<IdentityModelTypeMapping>> for IdentityModelType {
    type Expression = Bound<Nullable<IdentityModelTypeMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<IdentityModelTypeMapping> for &'a IdentityModelType {
    type Expression = Bound<IdentityModelTypeMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<Nullable<IdentityModelTypeMapping>> for &'a IdentityModelType {
    type Expression = Bound<Nullable<IdentityModelTypeMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<IdentityModelTypeMapping> for &'a &'b IdentityModelType {
    type Expression = Bound<IdentityModelTypeMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<Nullable<IdentityModelTypeMapping>> for &'a &'b IdentityModelType {
    type Expression = Bound<Nullable<IdentityModelTypeMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<DB: Backend> ToSql<IdentityModelTypeMapping, DB> for IdentityModelType {
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        match self {
            IdentityModelType::Key => out.write_all(b"key")?,
            IdentityModelType::User => out.write_all(b"user")?,
        }
        Ok(IsNull::No)
    }
}

impl<DB> ToSql<Nullable<IdentityModelTypeMapping>, DB> for IdentityModelType
where
    DB: Backend,
    Self: ToSql<IdentityModelTypeMapping, DB>,
{
    fn to_sql<W: ::std::io::Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        ToSql::<IdentityModelTypeMapping, DB>::to_sql(self, out)
    }
}

impl<DB> Queryable<IdentityModelTypeMapping, DB> for IdentityModelType
where
    DB: Backend + HasSqlType<IdentityModelTypeMapping>,
    IdentityModelType: FromSql<IdentityModelTypeMapping, DB>,
{
    type Row = Self;

    fn build(row: Self::Row) -> Self {
        row
    }
}

impl<DB> FromSqlRow<IdentityModelTypeMapping, DB> for IdentityModelType
where
    DB: Backend,
    IdentityModelType: FromSql<IdentityModelTypeMapping, DB>,
{
    fn build_from_row<T: Row<DB>>(row: &mut T) -> deserialize::Result<Self> {
        FromSql::<IdentityModelTypeMapping, DB>::from_sql(row.take())
    }
}

#[cfg(feature = "postgres")]
impl FromSql<IdentityModelTypeMapping, Pg> for IdentityModelType {
    fn from_sql(bytes: Option<&<Pg as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes {
            Some(b"key") => Ok(IdentityModelType::Key),
            Some(b"user") => Ok(IdentityModelType::User),
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
impl HasSqlType<IdentityModelTypeMapping> for Pg {
    fn metadata(lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        lookup.lookup_type("identity_type")
    }
}

#[cfg(feature = "sqlite")]
impl FromSql<IdentityModelTypeMapping, Sqlite> for IdentityModelType {
    fn from_sql(bytes: Option<&<Sqlite as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes.map(|v| v.read_blob()) {
            Some(b"key") => Ok(IdentityModelType::Key),
            Some(b"user") => Ok(IdentityModelType::User),
            Some(blob) => {
                Err(format!("Unexpected variant: {}", String::from_utf8_lossy(blob)).into())
            }
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

#[cfg(feature = "sqlite")]
impl HasSqlType<IdentityModelTypeMapping> for Sqlite {
    fn metadata(_lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        diesel::sqlite::SqliteType::Text
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable)]
#[table_name = "rbac_identities"]
#[primary_key(identity)]
pub(super) struct IdentityModel {
    pub identity: String,
    pub identity_type: IdentityModelType,
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable)]
#[table_name = "rbac_assignments"]
#[belongs_to(IdentityModel, foreign_key = "identity")]
#[primary_key(identity, role_id)]
pub(super) struct AssignmentModel {
    pub identity: String,
    pub role_id: String,
}
