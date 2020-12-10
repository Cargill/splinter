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

use diesel::prelude::*;

use crate::oauth::store::{
    diesel::{models::OAuthInflightRequest, schema::oauth_inflight_request},
    InflightOAuthRequestStoreError,
};

use super::InflightOAuthRequestOperations;

pub(in crate::oauth::store::diesel) trait InflightOAuthRequestStoreRemoveRequestOperation {
    fn remove_request(
        &self,
        request_id: &str,
    ) -> Result<Option<OAuthInflightRequest>, InflightOAuthRequestStoreError>;
}

impl<'a, C> InflightOAuthRequestStoreRemoveRequestOperation
    for InflightOAuthRequestOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn remove_request(
        &self,
        request_id: &str,
    ) -> Result<Option<OAuthInflightRequest>, InflightOAuthRequestStoreError> {
        self.conn
            .transaction::<Option<OAuthInflightRequest>, diesel::result::Error, _>(|| {
                let request = oauth_inflight_request::table
                    .filter(oauth_inflight_request::id.eq(request_id))
                    .first::<OAuthInflightRequest>(self.conn)
                    .optional()?;

                if request.is_some() {
                    diesel::delete(
                        oauth_inflight_request::table
                            .filter(oauth_inflight_request::id.eq(request_id)),
                    )
                    .execute(self.conn)?;
                }

                Ok(request)
            })
            .map_err(InflightOAuthRequestStoreError::from)
    }
}
