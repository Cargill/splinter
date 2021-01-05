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

use diesel::{dsl::delete, prelude::*};

use crate::rest_api::auth::roles::store::{
    diesel::schema::{assignments, identities},
    Identity, RoleBasedAuthorizationStoreError,
};

use super::RoleBasedAuthorizationStoreOperations;

pub trait RoleBasedAuthorizationStoreRemoveAssignment {
    fn remove_assignment(
        &self,
        identity: &Identity,
    ) -> Result<(), RoleBasedAuthorizationStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> RoleBasedAuthorizationStoreRemoveAssignment
    for RoleBasedAuthorizationStoreOperations<'a, diesel::sqlite::SqliteConnection>
{
    fn remove_assignment(
        &self,
        identity: &Identity,
    ) -> Result<(), RoleBasedAuthorizationStoreError> {
        let search_identity = match identity {
            Identity::Key(ref key) => key,
            Identity::User(ref user_id) => user_id,
        };
        self.conn.transaction::<_, _, _>(|| {
            delete(assignments::table.filter(assignments::identity.eq(search_identity)))
                .execute(self.conn)?;
            delete(identities::table.filter(identities::identity.eq(search_identity)))
                .execute(self.conn)?;

            Ok(())
        })
    }
}
