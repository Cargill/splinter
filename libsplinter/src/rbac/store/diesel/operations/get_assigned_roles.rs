// Copyright 2018-2022 Cargill Incorporated
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

use crate::rbac::store::{
    diesel::{
        models::{
            AssignmentModel, IdentityModel, IdentityModelType, IdentityModelTypeMapping, RoleModel,
            RolePermissionModel,
        },
        schema::{rbac_identities, rbac_roles},
    },
    Identity, Role, RoleBasedAuthorizationStoreError,
};

use super::RoleBasedAuthorizationStoreOperations;

pub trait RoleBasedAuthorizationStoreGetAssignedRoles {
    fn get_assigned_roles(
        &self,
        identity: &Identity,
    ) -> Result<Box<dyn ExactSizeIterator<Item = Role>>, RoleBasedAuthorizationStoreError>;
}

impl<'a, C> RoleBasedAuthorizationStoreGetAssignedRoles
    for RoleBasedAuthorizationStoreOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
    i16: diesel::deserialize::FromSql<diesel::sql_types::SmallInt, C::Backend>,
    <C as diesel::Connection>::Backend: diesel::types::HasSqlType<IdentityModelTypeMapping>,
    IdentityModelType: diesel::deserialize::FromSql<IdentityModelTypeMapping, C::Backend>,
{
    fn get_assigned_roles(
        &self,
        identity: &Identity,
    ) -> Result<Box<dyn ExactSizeIterator<Item = Role>>, RoleBasedAuthorizationStoreError> {
        let search_identity = match identity {
            Identity::Key(ref key) => key,
            Identity::User(ref user_id) => user_id,
        };
        self.conn
            .transaction::<Box<dyn ExactSizeIterator<Item = Role>>, _, _>(|| {
                let identities = rbac_identities::table
                    .filter(rbac_identities::identity.eq(search_identity))
                    .load::<IdentityModel>(self.conn)?;

                let role_ids = AssignmentModel::belonging_to(&identities)
                    .load::<AssignmentModel>(self.conn)?
                    .into_iter()
                    .map(|assignment| assignment.role_id)
                    .collect::<Vec<_>>();

                let roles = rbac_roles::table
                    .filter(rbac_roles::id.eq_any(role_ids))
                    .load::<RoleModel>(self.conn)?;

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
