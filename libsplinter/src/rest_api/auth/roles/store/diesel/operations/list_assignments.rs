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
        models::{AssignmentModel, IdentityModel},
        schema::identities,
    },
    Assignment, RoleBasedAuthorizationStoreError,
};

use super::RoleBasedAuthorizationStoreOperations;

pub trait RoleBasedAuthorizationStoreListAssignments {
    fn list_assignments(
        &self,
    ) -> Result<Box<dyn ExactSizeIterator<Item = Assignment>>, RoleBasedAuthorizationStoreError>;
}

impl<'a, C> RoleBasedAuthorizationStoreListAssignments
    for RoleBasedAuthorizationStoreOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
    i16: diesel::deserialize::FromSql<diesel::sql_types::SmallInt, C::Backend>,
{
    fn list_assignments(
        &self,
    ) -> Result<Box<dyn ExactSizeIterator<Item = Assignment>>, RoleBasedAuthorizationStoreError>
    {
        self.conn
            .transaction::<Box<dyn ExactSizeIterator<Item = Assignment>>, _, _>(|| {
                let identities = identities::table.load::<IdentityModel>(self.conn)?;

                let assignments = AssignmentModel::belonging_to(&identities)
                    .load::<AssignmentModel>(self.conn)?
                    .grouped_by(&identities);

                Ok(Box::new(
                    identities
                        .into_iter()
                        .zip(assignments)
                        .map(|models| models.try_into())
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(RoleBasedAuthorizationStoreError::from)?
                        .into_iter(),
                ))
            })
    }
}
