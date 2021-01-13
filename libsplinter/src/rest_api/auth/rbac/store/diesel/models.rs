// Copyright 2018-2020 Cargill Incorporated
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
    deserialize::{self, FromSql},
    expression::{helper_types::AsExprOf, AsExpression},
    serialize::{self, Output, ToSql},
    sql_types::SmallInt,
};

use super::schema::{assignments, identities, role_permissions, roles};

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable)]
#[table_name = "roles"]
#[primary_key(id)]
pub(super) struct RoleModel {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable)]
#[table_name = "role_permissions"]
#[belongs_to(RoleModel, foreign_key = "role_id")]
#[primary_key(role_id, permission)]
pub(super) struct RolePermissionModel {
    pub role_id: String,
    pub permission: String,
}

#[repr(i16)]
#[derive(Debug, Copy, Clone, PartialEq, FromSqlRow)]
pub(super) enum IdentityModelType {
    Key = 1,
    User = 2,
}

impl<DB> ToSql<SmallInt, DB> for IdentityModelType
where
    DB: Backend,
    i16: ToSql<SmallInt, DB>,
{
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        (*self as i16).to_sql(out)
    }
}

impl AsExpression<SmallInt> for IdentityModelType {
    type Expression = AsExprOf<i16, SmallInt>;

    fn as_expression(self) -> Self::Expression {
        <i16 as AsExpression<SmallInt>>::as_expression(self as i16)
    }
}

impl<'a> AsExpression<SmallInt> for &'a IdentityModelType {
    type Expression = AsExprOf<i16, SmallInt>;

    fn as_expression(self) -> Self::Expression {
        <i16 as AsExpression<SmallInt>>::as_expression((*self) as i16)
    }
}

impl<DB> FromSql<SmallInt, DB> for IdentityModelType
where
    DB: Backend,
    i16: FromSql<SmallInt, DB>,
{
    fn from_sql(bytes: Option<&DB::RawValue>) -> deserialize::Result<Self> {
        match i16::from_sql(bytes)? {
            1 => Ok(IdentityModelType::Key),
            2 => Ok(IdentityModelType::User),
            int => Err(format!("Invalid identity type {}", int).into()),
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable)]
#[table_name = "identities"]
#[primary_key(identity)]
pub(super) struct IdentityModel {
    pub identity: String,
    pub identity_type: IdentityModelType,
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable)]
#[table_name = "assignments"]
#[belongs_to(IdentityModel, foreign_key = "identity")]
#[primary_key(identity, role_id)]
pub(super) struct AssignmentModel {
    pub identity: String,
    pub role_id: String,
}
