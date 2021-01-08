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

//! This module defines the store trait for roles and their assignments to identities.

#[cfg(feature = "diesel")]
mod diesel;
mod error;

use crate::error::InvalidStateError;

#[cfg(feature = "diesel")]
pub use self::diesel::DieselRoleBasedAuthorizationStore;

pub use error::RoleBasedAuthorizationStoreError;

/// A Role is a named set of permissions.
pub struct Role {
    id: String,
    display_name: String,
    permissions: Vec<String>,
}

impl Role {
    /// Returns the role's id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the role's display name.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Returns the role's permissions.
    pub fn permissions(&self) -> &[String] {
        &self.permissions
    }

    /// Convert this role back into a builder, in order to update its values.
    pub fn into_update_builder(self) -> RoleUpdateBuilder {
        RoleUpdateBuilder {
            id: self.id,
            display_name: Some(self.display_name),
            permissions: self.permissions,
        }
    }

    /// Converts this role into it's constituent parts.  These parts are in the tuple:
    /// `(id, display_name, permissions)`.
    pub fn into_parts(self) -> (String, String, Vec<String>) {
        (self.id, self.display_name, self.permissions)
    }
}

/// A builder to create new roles.
#[derive(Default)]
pub struct RoleBuilder {
    id: Option<String>,
    display_name: Option<String>,
    permissions: Vec<String>,
}

impl RoleBuilder {
    /// Constructs a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the ID for the new role.
    pub fn with_id(mut self, id: String) -> Self {
        self.id = Some(id);
        self
    }

    /// Sets the display name for the new role.
    pub fn with_display_name(mut self, display_name: String) -> Self {
        self.display_name = Some(display_name);
        self
    }

    /// Sets the permissions for the new role.
    pub fn with_permissions(mut self, permissions: Vec<String>) -> Self {
        self.permissions = permissions;
        self
    }

    /// Builds the new Role.
    ///
    /// # Errors
    ///
    /// Returns an [`InvalidStateError`] under the following conditions:
    /// * no ID or an empty ID was provided
    /// * no display name or an empty display name was provided
    /// * empty permissions were provided
    pub fn build(self) -> Result<Role, InvalidStateError> {
        if self.permissions.is_empty() {
            return Err(InvalidStateError::with_message(
                "A role requires at least one permission".into(),
            ));
        }

        let id = self
            .id
            .ok_or_else(|| InvalidStateError::with_message("A role requires an id field".into()))?;
        if id.is_empty() {
            return Err(InvalidStateError::with_message(
                "A role requires a non-empty id field".into(),
            ));
        }
        let display_name = self.display_name.ok_or_else(|| {
            InvalidStateError::with_message("A role requires a display_name field".into())
        })?;

        if display_name.is_empty() {
            return Err(InvalidStateError::with_message(
                "A role requires a non-empty display_name field".into(),
            ));
        }

        Ok(Role {
            id,
            display_name,
            permissions: self.permissions,
        })
    }
}

/// Updates an existing role.
///
/// This builder only allows the updatable fields to be modified.
pub struct RoleUpdateBuilder {
    id: String,
    display_name: Option<String>,
    permissions: Vec<String>,
}

impl RoleUpdateBuilder {
    /// Updates the display name for the new role.
    pub fn with_display_name(mut self, display_name: String) -> Self {
        self.display_name = Some(display_name);
        self
    }

    /// Updates the permissions for the new role.
    pub fn with_permissions(mut self, permissions: Vec<String>) -> Self {
        self.permissions = permissions;
        self
    }

    /// Builds the updated Role.
    ///
    /// # Errors
    ///
    /// Returns an [`InvalidStateError`] under the following conditions:
    /// * an empty display name was provided
    /// * empty permissions were provided
    pub fn build(self) -> Result<Role, InvalidStateError> {
        if self.permissions.is_empty() {
            return Err(InvalidStateError::with_message(
                "A role requires at least one permission".into(),
            ));
        }

        let display_name = self.display_name.ok_or_else(|| {
            InvalidStateError::with_message("A role requires a display_name field".into())
        })?;

        if display_name.is_empty() {
            return Err(InvalidStateError::with_message(
                "A role requires a non-empty display_name field".into(),
            ));
        }

        Ok(Role {
            id: self.id,
            display_name,
            permissions: self.permissions,
        })
    }
}

/// An identity that may be assigned roles.
#[derive(Debug, PartialEq)]
pub enum Identity {
    /// A public key-based identity.
    Key(String),
    /// A user ID-based identity.
    User(String),
}

/// An assignment of roles to a particular identity.
pub struct Assignment {
    identity: Identity,
    roles: Vec<String>,
}

impl Assignment {
    /// Returns the identity that has been assigned this set of roles.
    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    /// Returns the assigned roles IDs.
    pub fn roles(&self) -> &[String] {
        &self.roles
    }

    /// Convert this assignment back into a builder, in order to update its values.
    pub fn into_update_builder(self) -> AssignmentUpdateBuilder {
        let Assignment { identity, roles } = self;
        AssignmentUpdateBuilder { identity, roles }
    }

    /// Converts this assignment into it's constituent parts.  These parts are in the tuple:
    /// `(identity, roles)`.
    pub fn into_parts(self) -> (Identity, Vec<String>) {
        (self.identity, self.roles)
    }
}

/// Constructs new Assignments.
#[derive(Default)]
pub struct AssignmentBuilder {
    identity: Option<Identity>,
    roles: Vec<String>,
}

impl AssignmentBuilder {
    /// Constructs a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the identity for the assignment.
    pub fn with_identity(mut self, identity: Identity) -> Self {
        self.identity = Some(identity);
        self
    }

    /// Sets the assigned roles.
    pub fn with_roles(mut self, roles: Vec<String>) -> Self {
        self.roles = roles;
        self
    }

    /// Builds a new assignment.
    ///
    /// # Errors
    ///
    /// Returns an [`InvalidStateError`] under the following conditions:
    /// * no identity was provided
    /// * no roles were provided
    pub fn build(self) -> Result<Assignment, InvalidStateError> {
        if self.roles.is_empty() {
            return Err(InvalidStateError::with_message(
                "An assignment requires at least one role".into(),
            ));
        }

        Ok(Assignment {
            identity: self.identity.ok_or_else(|| {
                InvalidStateError::with_message("An assignment requires an identity field".into())
            })?,
            roles: self.roles,
        })
    }
}

/// Updates an existing assignment.
///
/// This builder only allows the updatable fields to be modified.
pub struct AssignmentUpdateBuilder {
    identity: Identity,
    roles: Vec<String>,
}

impl AssignmentUpdateBuilder {
    /// Updates the assigned roles.
    pub fn with_roles(mut self, roles: Vec<String>) -> Self {
        self.roles = roles;
        self
    }

    /// Builds the updated assignment.
    ///
    /// # Errors
    ///
    /// Returns an [`InvalidStateError`] under the following conditions:
    /// * no roles were provided
    pub fn build(self) -> Result<Assignment, InvalidStateError> {
        if self.roles.is_empty() {
            return Err(InvalidStateError::with_message(
                "An assignment requires at least one role".into(),
            ));
        }

        Ok(Assignment {
            identity: self.identity,
            roles: self.roles,
        })
    }
}

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
    /// Returns a `InvalidState` error if the role does not exist.
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
    /// Returns a `InvalidState` error if the assignment does not exist.
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
