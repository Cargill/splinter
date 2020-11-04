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

use diesel::{dsl::update, prelude::*, result::DatabaseErrorKind, result::Error as DieselError};

use crate::error::InternalError;

use crate::biome::oauth::store::{diesel::schema::oauth_user, OAuthUser, OAuthUserStoreError};

use super::OAuthUserStoreOperations;

pub(in crate::biome::oauth) trait OAuthUserStoreUpdateOAuthUserOperation {
    fn update_oauth_user(&self, oauth_user: OAuthUser) -> Result<(), OAuthUserStoreError>;
}

impl<'a, C> OAuthUserStoreUpdateOAuthUserOperation for OAuthUserStoreOperations<'a, C>
where
    C: diesel::Connection,
{
    fn update_oauth_user(&self, oauth_user: OAuthUser) -> Result<(), OAuthUserStoreError> {
        update(oauth_user::table.filter(oauth_user::user_id.eq(oauth_user.user_id())))
            .set((
                oauth_user::access_token.eq(oauth_user.access_token()),
                oauth_user::refresh_token.eq(&oauth_user.refresh_token()),
            ))
            .execute(self.conn)
            .map(|_| ())
            .map_err(|err| match err {
                DieselError::DatabaseError(ref kind, _) => match kind {
                    DatabaseErrorKind::UniqueViolation | DatabaseErrorKind::ForeignKeyViolation => {
                        OAuthUserStoreError::ConstraintViolation(Box::new(err))
                    }
                    _ => OAuthUserStoreError::InternalError(InternalError::from_source(Box::new(
                        err,
                    ))),
                },
                _ => OAuthUserStoreError::InternalError(InternalError::from_source(Box::new(err))),
            })
    }
}
