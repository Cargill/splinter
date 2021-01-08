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

use crate::rest_api::auth::roles::store::{
    diesel::{
        models::{RoleModel, RolePermissionModel},
        schema::{role_permissions, roles},
    },
    Role, RoleBasedAuthorizationStoreError,
};

use super::RoleBasedAuthorizationStoreOperations;

pub trait RoleBasedAuthorizationStoreAddRole {
    fn add_role(&self, role: Role) -> Result<(), RoleBasedAuthorizationStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> RoleBasedAuthorizationStoreAddRole
    for RoleBasedAuthorizationStoreOperations<'a, diesel::sqlite::SqliteConnection>
{
    fn add_role(&self, role: Role) -> Result<(), RoleBasedAuthorizationStoreError> {
        let (role, permissions): (RoleModel, Vec<RolePermissionModel>) = role.into();

        self.conn.transaction::<_, _, _>(|| {
            insert_into(roles::table).values(role).execute(self.conn)?;

            insert_into(role_permissions::table)
                .values(permissions)
                .execute(self.conn)?;

            Ok(())
        })
    }
}

#[cfg(feature = "role-based-authorization-store-postgres")]
impl<'a> RoleBasedAuthorizationStoreAddRole
    for RoleBasedAuthorizationStoreOperations<'a, diesel::pg::PgConnection>
{
    fn add_role(&self, role: Role) -> Result<(), RoleBasedAuthorizationStoreError> {
        let (role, permissions): (RoleModel, Vec<RolePermissionModel>) = role.into();
        self.conn.transaction::<_, _, _>(|| {
            insert_into(roles::table).values(role).execute(self.conn)?;

            insert_into(role_permissions::table)
                .values(permissions)
                .execute(self.conn)?;

            Ok(())
        })
    }
}
