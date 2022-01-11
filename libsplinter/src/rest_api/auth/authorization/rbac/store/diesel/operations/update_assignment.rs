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

use diesel::{
    dsl::{delete, insert_into},
    prelude::*,
};

use crate::error::{ConstraintViolationError, ConstraintViolationType};
use crate::rest_api::auth::authorization::rbac::store::{
    diesel::{
        models::{AssignmentModel, IdentityModel},
        schema::{rbac_assignments, rbac_identities},
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
            let count = rbac_identities::table
                .filter(
                    rbac_identities::identity
                        .eq(&identity.identity)
                        .and(rbac_identities::identity_type.eq(identity.identity_type)),
                )
                .count()
                .get_result::<i64>(self.conn)?;

            if count == 0 {
                return Err(RoleBasedAuthorizationStoreError::ConstraintViolation(
                    ConstraintViolationError::with_violation_type(
                        ConstraintViolationType::NotFound,
                    ),
                ));
            }

            delete(
                rbac_assignments::table.filter(rbac_assignments::identity.eq(&identity.identity)),
            )
            .execute(self.conn)?;

            insert_into(rbac_assignments::table)
                .values(roles)
                .execute(self.conn)?;

            Ok(())
        })
    }
}

#[cfg(feature = "postgres")]
impl<'a> RoleBasedAuthorizationStoreUpdateAssignment
    for RoleBasedAuthorizationStoreOperations<'a, diesel::pg::PgConnection>
{
    fn update_assignment(
        &self,
        assignment: Assignment,
    ) -> Result<(), RoleBasedAuthorizationStoreError> {
        let (identity, roles): (IdentityModel, Vec<AssignmentModel>) = assignment.into();
        self.conn.transaction::<_, _, _>(|| {
            let count = rbac_identities::table
                .filter(
                    rbac_identities::identity
                        .eq(&identity.identity)
                        .and(rbac_identities::identity_type.eq(identity.identity_type)),
                )
                .count()
                .get_result::<i64>(self.conn)?;

            if count == 0 {
                return Err(RoleBasedAuthorizationStoreError::ConstraintViolation(
                    ConstraintViolationError::with_violation_type(
                        ConstraintViolationType::NotFound,
                    ),
                ));
            }

            delete(
                rbac_assignments::table.filter(rbac_assignments::identity.eq(&identity.identity)),
            )
            .execute(self.conn)?;

            insert_into(rbac_assignments::table)
                .values(roles)
                .execute(self.conn)?;

            Ok(())
        })
    }
}
