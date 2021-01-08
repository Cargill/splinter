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

pub trait RoleBasedAuthorizationStoreListRoles {
    fn list_roles(
        &self,
    ) -> Result<Box<dyn ExactSizeIterator<Item = Role>>, RoleBasedAuthorizationStoreError>;
}

impl<'a, C> RoleBasedAuthorizationStoreListRoles for RoleBasedAuthorizationStoreOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn list_roles(
        &self,
    ) -> Result<Box<dyn ExactSizeIterator<Item = Role>>, RoleBasedAuthorizationStoreError> {
        self.conn
            .transaction::<Box<dyn ExactSizeIterator<Item = Role>>, _, _>(|| {
                let roles = roles::table.load::<RoleModel>(self.conn)?;

                let perms = RolePermissionModel::belonging_to(&roles)
                    .load::<RolePermissionModel>(self.conn)?
                    .grouped_by(&roles);

                Ok(Box::new(
                    roles
                        .into_iter()
                        .zip(perms)
                        .map(|models| models.try_into())
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(RoleBasedAuthorizationStoreError::from)?
                        .into_iter(),
                ))
            })
    }
}
