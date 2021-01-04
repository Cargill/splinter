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

mod models;
mod schema;

use diesel::r2d2::{ConnectionManager, Pool};

use super::{
    Assignment, Identity, Role, RoleBasedAuthorizationStore, RoleBasedAuthorizationStoreError,
    RoleBuilder,
};

/// A database-backed [RoleBasedAuthorizationStore], powered by [diesel].
pub struct DieselRoleBasedAuthorizationStore<C: diesel::Connection + 'static> {
    connection_pool: Pool<ConnectionManager<C>>,
}

impl<C: diesel::Connection + 'static> DieselRoleBasedAuthorizationStore<C> {
    pub fn new(connection_pool: Pool<ConnectionManager<C>>) -> Self {
        Self { connection_pool }
    }
}

#[cfg(feature = "sqlite")]
impl RoleBasedAuthorizationStore
    for DieselRoleBasedAuthorizationStore<diesel::sqlite::SqliteConnection>
{
    /// Returns the role for the given ID, if one exists.
    fn get_role(&self, id: &str) -> Result<Option<Role>, RoleBasedAuthorizationStoreError> {
        todo!()
    }

    /// Lists all roles.
    fn list_roles(
        &self,
    ) -> Result<Box<dyn ExactSizeIterator<Item = Role>>, RoleBasedAuthorizationStoreError> {
        todo!()
    }

    /// Adds a role.
    ///
    /// # Errors
    ///
    /// Returns a `ConstraintViolation` error if a duplicate role ID is added.
    fn add_role(&self, role: Role) -> Result<(), RoleBasedAuthorizationStoreError> {
        todo!()
    }

    /// Updates a role.
    ///
    /// # Errors
    ///
    /// Returns a `InvalidState` error if the role does not exist.
    fn update_role(&self, role: Role) -> Result<(), RoleBasedAuthorizationStoreError> {
        todo!()
    }

    /// Removes a role.
    ///
    /// # Errors
    ///
    /// Returns a `InvalidState` error if the role does not exist.
    fn remove_role(&self, role_id: &str) -> Result<(), RoleBasedAuthorizationStoreError> {
        todo!()
    }

    /// Returns the role for the given Identity, if one exists.
    fn get_assignment(
        &self,
        identity: &Identity,
    ) -> Result<Option<Assignment>, RoleBasedAuthorizationStoreError> {
        todo!()
    }

    /// Lists all assignments.
    fn list_assignments(
        &self,
    ) -> Result<Box<dyn ExactSizeIterator<Item = Assignment>>, RoleBasedAuthorizationStoreError>
    {
        todo!()
    }

    /// Adds an assignment.
    ///
    /// # Errors
    ///
    /// Returns a `ConstraintViolation` error if there is a duplicate assignment of a role to an
    /// identity.
    fn add_assignment(
        &self,
        assignment: Assignment,
    ) -> Result<(), RoleBasedAuthorizationStoreError> {
        todo!()
    }

    /// Updates an assignment.
    ///
    /// # Errors
    ///
    /// Returns a `InvalidState` error if the assignment does not exist.
    fn update_assignment(
        &self,
        assignment: Assignment,
    ) -> Result<(), RoleBasedAuthorizationStoreError> {
        todo!()
    }

    /// Removes an assignment.
    ///
    /// # Errors
    ///
    /// Returns a `InvalidState` error if the assignment does not exist.
    fn remove_assignment(
        &self,
        identity: &Identity,
    ) -> Result<(), RoleBasedAuthorizationStoreError> {
        todo!()
    }

    /// Clone into a boxed, dynamically dispatched store
    fn clone_box(&self) -> Box<dyn RoleBasedAuthorizationStore> {
        Box::new(DieselRoleBasedAuthorizationStore {
            connection_pool: self.connection_pool.clone(),
        })
    }
}
