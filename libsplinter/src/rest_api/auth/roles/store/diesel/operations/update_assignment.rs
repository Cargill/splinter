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

use diesel::{
    dsl::{delete, insert_into},
    prelude::*,
};

use crate::rest_api::auth::roles::store::{
    diesel::{
        models::{AssignmentModel, IdentityModel},
        schema::assignments,
    },
    Assignment, RoleBasedAuthorizationStoreError,
};

use super::RoleBasedAuthorizationStoreOperations;

pub trait RoleBasedAuthorizationStoreUpdateAssignment {
    fn update_assignment(
        &self,
        assignment: Assignment,
    ) -> Result<(), RoleBasedAuthorizationStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> RoleBasedAuthorizationStoreUpdateAssignment
    for RoleBasedAuthorizationStoreOperations<'a, diesel::sqlite::SqliteConnection>
{
    fn update_assignment(
        &self,
        assignment: Assignment,
    ) -> Result<(), RoleBasedAuthorizationStoreError> {
        let (identity, roles): (IdentityModel, Vec<AssignmentModel>) = assignment.into();
        self.conn.transaction::<_, _, _>(|| {
            delete(assignments::table.filter(assignments::identity.eq(&identity.identity)))
                .execute(self.conn)?;

            insert_into(assignments::table)
                .values(roles)
                .execute(self.conn)?;

            Ok(())
        })
    }
}
