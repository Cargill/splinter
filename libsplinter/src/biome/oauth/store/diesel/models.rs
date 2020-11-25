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

use crate::biome::user::store::diesel::models::UserModel;

use super::schema::oauth_user;

#[repr(i16)]
#[derive(Debug, PartialEq, FromSqlRow, Clone, Copy)]
pub enum ProviderId {
    Github = 1,
    OpenId = 2,
}

impl<DB> ToSql<SmallInt, DB> for ProviderId
where
    DB: Backend,
    i16: ToSql<SmallInt, DB>,
{
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        (*self as i16).to_sql(out)
    }
}

impl AsExpression<SmallInt> for ProviderId {
    type Expression = AsExprOf<i16, SmallInt>;

    fn as_expression(self) -> Self::Expression {
        <i16 as AsExpression<SmallInt>>::as_expression(self as i16)
    }
}

impl<'a> AsExpression<SmallInt> for &'a ProviderId {
    type Expression = AsExprOf<i16, SmallInt>;

    fn as_expression(self) -> Self::Expression {
        <i16 as AsExpression<SmallInt>>::as_expression((*self) as i16)
    }
}

impl<DB> FromSql<SmallInt, DB> for ProviderId
where
    DB: Backend,
    i16: FromSql<SmallInt, DB>,
{
    fn from_sql(bytes: Option<&DB::RawValue>) -> deserialize::Result<Self> {
        match i16::from_sql(bytes)? {
            1 => Ok(ProviderId::Github),
            2 => Ok(ProviderId::OpenId),
            int => Err(format!("Invalid provider {}", int).into()),
        }
    }
}

#[derive(Queryable, Identifiable, Associations, PartialEq, Debug)]
#[table_name = "oauth_user"]
#[belongs_to(UserModel, foreign_key = "user_id")]
pub struct OAuthUserModel {
    pub id: i64,
    pub user_id: String,
    pub provider_user_ref: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub provider_id: ProviderId,
}

#[derive(Insertable, PartialEq, Debug)]
#[table_name = "oauth_user"]
pub struct NewOAuthUserModel<'a> {
    pub user_id: &'a str,
    pub provider_user_ref: &'a str,
    pub access_token: Option<&'a str>,
    pub refresh_token: Option<&'a str>,
    pub provider_id: ProviderId,
}
