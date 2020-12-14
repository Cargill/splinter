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

use diesel::{dsl::insert_into, prelude::*};

use crate::oauth::store::{
    diesel::{models::OAuthInflightRequest, schema::oauth_inflight_request},
    InflightOAuthRequestStoreError,
};

use super::InflightOAuthRequestOperations;

pub(in crate::oauth::store::diesel) trait InflightOAuthRequestStoreInsertRequestOperation {
    fn insert_request(
        &self,
        request: OAuthInflightRequest,
    ) -> Result<(), InflightOAuthRequestStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> InflightOAuthRequestStoreInsertRequestOperation
    for InflightOAuthRequestOperations<'a, diesel::sqlite::SqliteConnection>
{
    fn insert_request(
        &self,
        request: OAuthInflightRequest,
    ) -> Result<(), InflightOAuthRequestStoreError> {
        insert_into(oauth_inflight_request::table)
            .values(request)
            .execute(self.conn)
            .map(|_| ())
            .map_err(InflightOAuthRequestStoreError::from)
    }
}

#[cfg(feature = "postgres")]
impl<'a> InflightOAuthRequestStoreInsertRequestOperation
    for InflightOAuthRequestOperations<'a, diesel::pg::PgConnection>
{
    fn insert_request(
        &self,
        request: OAuthInflightRequest,
    ) -> Result<(), InflightOAuthRequestStoreError> {
        insert_into(oauth_inflight_request::table)
            .values(request)
            .execute(self.conn)
            .map(|_| ())
            .map_err(InflightOAuthRequestStoreError::from)
    }
}
