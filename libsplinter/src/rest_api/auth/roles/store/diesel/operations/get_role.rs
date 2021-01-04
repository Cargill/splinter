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

use std::convert::TryInto;

use diesel::prelude::*;

use crate::rest_api::auth::roles::store::{
    diesel::{
        models::{RoleModel, RolePermissionModel},
        schema::roles,
    },
    Role, RoleBasedAuthorizationStoreError,
};

use super::RoleBasedAuthorizationStoreOperations;

pub trait RoleBasedAuthorizationStoreGetRole {
    fn get_role(&self, search_id: &str) -> Result<Option<Role>, RoleBasedAuthorizationStoreError>;
}

impl<'a, C> RoleBasedAuthorizationStoreGetRole for RoleBasedAuthorizationStoreOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn get_role(&self, search_id: &str) -> Result<Option<Role>, RoleBasedAuthorizationStoreError> {
        self.conn.transaction(|| {
            let roles = roles::table
                .filter(roles::id.eq(search_id))
                .load::<RoleModel>(self.conn)?;

            let perms = RolePermissionModel::belonging_to(&roles)
                .load::<RolePermissionModel>(self.conn)?
                .grouped_by(&roles);

            roles
                .into_iter()
                .zip(perms)
                .next()
                .map(|models| models.try_into())
                .transpose()
                .map_err(RoleBasedAuthorizationStoreError::from)
        })
    }
}
