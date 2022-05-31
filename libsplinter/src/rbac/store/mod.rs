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

//! This module defines the store trait for roles and their assignments to identities.

mod assignment;
#[cfg(feature = "diesel")]
mod diesel;
mod error;
mod identity;
mod role;

pub use assignment::{Assignment, AssignmentBuilder, AssignmentUpdateBuilder};
pub use identity::Identity;
pub use role::{Role, RoleBuilder, RoleUpdateBuilder};

#[cfg(feature = "diesel")]
pub use self::diesel::DieselRoleBasedAuthorizationStore;

pub use error::RoleBasedAuthorizationStoreError;

pub const ADMIN_ROLE_ID: &str = "admin";

/// Defines methods for CRUD operations on Role and assignment data.
pub trait RoleBasedAuthorizationStore: Send + Sync {
    /// Returns the role for the given ID, if one exists.
    fn get_role(&self, id: &str) -> Result<Option<Role>, RoleBasedAuthorizationStoreError>;

    /// Lists all roles.
    fn list_roles(
        &self,
    ) -> Result<Box<dyn ExactSizeIterator<Item = Role>>, RoleBasedAuthorizationStoreError>;

    /// Adds a role.
    ///
    /// # Errors
    ///
    /// Returns a `ConstraintViolation` error if a duplicate role ID is added.
    fn add_role(&self, role: Role) -> Result<(), RoleBasedAuthorizationStoreError>;

    /// Updates a role.
    ///
    /// # Errors
    ///
    /// Returns a `ConstraintViolation` error if the role does not exist.
    fn update_role(&self, role: Role) -> Result<(), RoleBasedAuthorizationStoreError>;

    /// Removes a role.
    ///
    /// # Errors
    ///
    /// Returns a `InvalidState` error if the role does not exist.
    fn remove_role(&self, role_id: &str) -> Result<(), RoleBasedAuthorizationStoreError>;

    /// Returns the role for the given Identity, if one exists.
    fn get_assignment(
        &self,
        identity: &Identity,
    ) -> Result<Option<Assignment>, RoleBasedAuthorizationStoreError>;

    /// Returns the assigned roles for the given Identity.
    fn get_assigned_roles(
        &self,
        identity: &Identity,
    ) -> Result<Box<dyn ExactSizeIterator<Item = Role>>, RoleBasedAuthorizationStoreError>;

    /// Lists all assignments.
    fn list_assignments(
        &self,
    ) -> Result<Box<dyn ExactSizeIterator<Item = Assignment>>, RoleBasedAuthorizationStoreError>;

    /// Adds an assignment.
    ///
    /// # Errors
    ///
    /// Returns a `ConstraintViolation` error if there is a duplicate assignment of a role to an
    /// identity.
    fn add_assignment(
        &self,
        assignment: Assignment,
    ) -> Result<(), RoleBasedAuthorizationStoreError>;

    /// Updates an assignment.
    ///
    /// # Errors
    ///
    /// Returns a `ConstraintViolation` error if the assignment does not exist.
    fn update_assignment(
        &self,
        assignment: Assignment,
    ) -> Result<(), RoleBasedAuthorizationStoreError>;

    /// Removes an assignment.
    ///
    /// # Errors
    ///
    /// Returns a `InvalidState` error if the assignment does not exist.
    fn remove_assignment(
        &self,
        identity: &Identity,
    ) -> Result<(), RoleBasedAuthorizationStoreError>;

    /// Clone into a boxed, dynamically dispatched store
    fn clone_box(&self) -> Box<dyn RoleBasedAuthorizationStore>;
}

impl Clone for Box<dyn RoleBasedAuthorizationStore> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
