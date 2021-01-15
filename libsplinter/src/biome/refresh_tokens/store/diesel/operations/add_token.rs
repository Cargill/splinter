// Copyright 2018-2021 Cargill Incorporated
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

use super::RefreshTokenStoreOperations;
use crate::biome::refresh_tokens::store::{
    diesel::{models::NewRefreshToken, schema::refresh_tokens},
    RefreshTokenError,
};
use crate::biome::user::store::diesel::{models::UserModel, schema::splinter_user};
use diesel::{dsl::insert_into, prelude::*, result::Error::NotFound};

pub(in crate::biome) trait RefreshTokenStoreAddTokenOperation {
    fn add_token(&self, user_id: &str, token: &str) -> Result<(), RefreshTokenError>;
}

impl<'a, C> RefreshTokenStoreAddTokenOperation for RefreshTokenStoreOperations<'a, C>
where
    C: diesel::Connection,
    <C as diesel::Connection>::Backend: diesel::backend::SupportsDefaultKeyword,
    <C as diesel::Connection>::Backend: 'static,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn add_token(&self, user_id: &str, token: &str) -> Result<(), RefreshTokenError> {
        splinter_user::table
            .filter(splinter_user::id.eq(&user_id))
            .first::<UserModel>(self.conn)
            .map_err(|err| {
                if err == NotFound {
                    RefreshTokenError::QueryError {
                        context: "Failed to check if user exists".into(),
                        source: Box::new(err),
                    }
                } else {
                    RefreshTokenError::NotFoundError(format!("User {} not found", user_id))
                }
            })?;

        insert_into(refresh_tokens::table)
            .values(NewRefreshToken { user_id, token })
            .execute(self.conn)
            .map_err(|err| RefreshTokenError::OperationError {
                context: "Failed to create token".to_string(),
                source: Box::new(err),
            })?;
        Ok(())
    }
}
