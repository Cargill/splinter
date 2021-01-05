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
    Assignment, Identity, RoleBasedAuthorizationStoreError,
};

use super::RoleBasedAuthorizationStoreOperations;

pub trait RoleBasedAuthorizationStoreGetAssignment {
    fn get_assignment(
        &self,
        identity: &Identity,
    ) -> Result<Option<Assignment>, RoleBasedAuthorizationStoreError>;
}

impl<'a, C> RoleBasedAuthorizationStoreGetAssignment
    for RoleBasedAuthorizationStoreOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
    i16: diesel::deserialize::FromSql<diesel::sql_types::SmallInt, C::Backend>,
{
    fn get_assignment(
        &self,
        identity: &Identity,
    ) -> Result<Option<Assignment>, RoleBasedAuthorizationStoreError> {
        let search_identity = match identity {
            Identity::Key(ref key) => key,
            Identity::User(ref user_id) => user_id,
        };
        self.conn.transaction(|| {
            let identities = identities::table
                .filter(identities::identity.eq(search_identity))
                .load::<IdentityModel>(self.conn)?;

            let assignments = AssignmentModel::belonging_to(&identities)
                .load::<AssignmentModel>(self.conn)?
                .grouped_by(&identities);

            identities
                .into_iter()
                .zip(assignments)
                .next()
                .map(|model| model.try_into())
                .transpose()
                .map_err(RoleBasedAuthorizationStoreError::from)
        })
    }
}
