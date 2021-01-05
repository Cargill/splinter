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
mod operations;
mod schema;

use std::convert::TryFrom;

use crate::error::{
    ConstraintViolationError, ConstraintViolationType, InternalError, InvalidStateError,
};

use diesel::r2d2::{ConnectionManager, Pool};

use super::{
    Assignment, AssignmentBuilder, Identity, Role, RoleBasedAuthorizationStore,
    RoleBasedAuthorizationStoreError, RoleBuilder,
};

use operations::add_assignment::RoleBasedAuthorizationStoreAddAssignment as _;
use operations::add_role::RoleBasedAuthorizationStoreAddRole as _;
use operations::get_assignment::RoleBasedAuthorizationStoreGetAssignment as _;
use operations::get_role::RoleBasedAuthorizationStoreGetRole as _;
use operations::list_assignments::RoleBasedAuthorizationStoreListAssignments as _;
use operations::list_roles::RoleBasedAuthorizationStoreListRoles as _;
use operations::remove_assignment::RoleBasedAuthorizationStoreRemoveAssignment as _;
use operations::remove_role::RoleBasedAuthorizationStoreRemoveRole as _;
use operations::update_assignment::RoleBasedAuthorizationStoreUpdateAssignment as _;
use operations::update_role::RoleBasedAuthorizationStoreUpdateRole as _;
use operations::RoleBasedAuthorizationStoreOperations;

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
        let connection = self.connection_pool.get()?;
        RoleBasedAuthorizationStoreOperations::new(&*connection).get_role(id)
    }

    /// Lists all roles.
    fn list_roles(
        &self,
    ) -> Result<Box<dyn ExactSizeIterator<Item = Role>>, RoleBasedAuthorizationStoreError> {
        let connection = self.connection_pool.get()?;
        RoleBasedAuthorizationStoreOperations::new(&*connection).list_roles()
    }

    /// Adds a role.
    ///
    /// # Errors
    ///
    /// Returns a `ConstraintViolation` error if a duplicate role ID is added.
    fn add_role(&self, role: Role) -> Result<(), RoleBasedAuthorizationStoreError> {
        let connection = self.connection_pool.get()?;
        RoleBasedAuthorizationStoreOperations::new(&*connection).add_role(role)
    }

    /// Updates a role.
    ///
    /// # Errors
    ///
    /// Returns a `InvalidState` error if the role does not exist.
    fn update_role(&self, role: Role) -> Result<(), RoleBasedAuthorizationStoreError> {
        let connection = self.connection_pool.get()?;
        RoleBasedAuthorizationStoreOperations::new(&*connection).update_role(role)
    }

    /// Removes a role.
    ///
    /// # Errors
    ///
    /// Returns a `InvalidState` error if the role does not exist.
    fn remove_role(&self, role_id: &str) -> Result<(), RoleBasedAuthorizationStoreError> {
        let connection = self.connection_pool.get()?;
        RoleBasedAuthorizationStoreOperations::new(&*connection).remove_role(role_id)
    }

    /// Returns the role for the given Identity, if one exists.
    fn get_assignment(
        &self,
        identity: &Identity,
    ) -> Result<Option<Assignment>, RoleBasedAuthorizationStoreError> {
        let connection = self.connection_pool.get()?;
        RoleBasedAuthorizationStoreOperations::new(&*connection).get_assignment(identity)
    }

    /// Lists all assignments.
    fn list_assignments(
        &self,
    ) -> Result<Box<dyn ExactSizeIterator<Item = Assignment>>, RoleBasedAuthorizationStoreError>
    {
        let connection = self.connection_pool.get()?;
        RoleBasedAuthorizationStoreOperations::new(&*connection).list_assignments()
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
        let connection = self.connection_pool.get()?;
        RoleBasedAuthorizationStoreOperations::new(&*connection).add_assignment(assignment)
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
        let connection = self.connection_pool.get()?;
        RoleBasedAuthorizationStoreOperations::new(&*connection).update_assignment(assignment)
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
        let connection = self.connection_pool.get()?;
        RoleBasedAuthorizationStoreOperations::new(&*connection).remove_assignment(identity)
    }

    /// Clone into a boxed, dynamically dispatched store
    fn clone_box(&self) -> Box<dyn RoleBasedAuthorizationStore> {
        Box::new(DieselRoleBasedAuthorizationStore {
            connection_pool: self.connection_pool.clone(),
        })
    }
}

impl From<Role> for (models::RoleModel, Vec<models::RolePermissionModel>) {
    fn from(role: Role) -> Self {
        let (id, display_name, permissions) = role.into_parts();

        let perm_models = permissions
            .into_iter()
            .map(|permission| models::RolePermissionModel {
                role_id: id.clone(),
                permission,
            })
            .collect::<Vec<_>>();
        (models::RoleModel { id, display_name }, perm_models)
    }
}

impl TryFrom<(models::RoleModel, Vec<models::RolePermissionModel>)> for Role {
    type Error = InvalidStateError;

    fn try_from(
        (role_model, perm_models): (models::RoleModel, Vec<models::RolePermissionModel>),
    ) -> Result<Self, Self::Error> {
        RoleBuilder::new()
            .with_id(role_model.id)
            .with_display_name(role_model.display_name)
            .with_permissions(
                perm_models
                    .into_iter()
                    .map(|perm| perm.permission)
                    .collect(),
            )
            .build()
    }
}

impl From<Assignment> for (models::IdentityModel, Vec<models::AssignmentModel>) {
    fn from(assignment: Assignment) -> Self {
        let (identity, roles) = assignment.into_parts();

        let identity_model = match identity {
            Identity::Key(identity) => models::IdentityModel {
                identity,
                identity_type: models::IdentityModelType::Key,
            },
            Identity::User(identity) => models::IdentityModel {
                identity,
                identity_type: models::IdentityModelType::User,
            },
        };

        let role_models = roles
            .into_iter()
            .map(|role_id| models::AssignmentModel {
                identity: identity_model.identity.clone(),
                role_id,
            })
            .collect::<Vec<_>>();

        (identity_model, role_models)
    }
}

impl TryFrom<(models::IdentityModel, Vec<models::AssignmentModel>)> for Assignment {
    type Error = InvalidStateError;

    fn try_from(
        (identity_model, assignments): (models::IdentityModel, Vec<models::AssignmentModel>),
    ) -> Result<Self, Self::Error> {
        let models::IdentityModel {
            identity,
            identity_type,
        } = identity_model;
        let identity = match identity_type {
            models::IdentityModelType::Key => Identity::Key(identity),
            models::IdentityModelType::User => Identity::User(identity),
        };
        AssignmentBuilder::new()
            .with_identity(identity)
            .with_roles(
                assignments
                    .into_iter()
                    .map(|assignment| assignment.role_id)
                    .collect(),
            )
            .build()
    }
}

impl From<diesel::result::Error> for RoleBasedAuthorizationStoreError {
    fn from(err: diesel::result::Error) -> Self {
        match err {
            diesel::result::Error::DatabaseError(ref kind, _) => match kind {
                diesel::result::DatabaseErrorKind::UniqueViolation => {
                    RoleBasedAuthorizationStoreError::ConstraintViolation(
                        ConstraintViolationError::from_source_with_violation_type(
                            ConstraintViolationType::Unique,
                            Box::new(err),
                        ),
                    )
                }
                _ => RoleBasedAuthorizationStoreError::InternalError(InternalError::from_source(
                    Box::new(err),
                )),
            },
            _ => RoleBasedAuthorizationStoreError::InternalError(InternalError::from_source(
                Box::new(err),
            )),
        }
    }
}

impl From<diesel::r2d2::PoolError> for RoleBasedAuthorizationStoreError {
    fn from(err: diesel::r2d2::PoolError) -> Self {
        RoleBasedAuthorizationStoreError::InternalError(InternalError::from_source(Box::new(err)))
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;

    use crate::rest_api::auth::roles::store::RoleBuilder;

    use crate::migrations::run_sqlite_migrations;

    use diesel::{
        prelude::*,
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };

    /// This tests verifies the following:
    /// 1. Adds a role via the store API
    /// 2. Verifies it has been added by getting the role via the store API
    #[test]
    fn sqlite_add_and_get_role() {
        let pool = create_connection_pool_and_migrate();

        let role_based_auth_store = DieselRoleBasedAuthorizationStore::new(pool);

        let stored_role = role_based_auth_store
            .get_role("test-role")
            .expect("Unable to lookup role by id");
        assert!(stored_role.is_none());

        let role = RoleBuilder::new()
            .with_id("test-role".into())
            .with_display_name("Test Role".into())
            .with_permissions(vec!["a".to_string(), "b".to_string(), "c".to_string()])
            .build()
            .expect("Unable to build role");

        role_based_auth_store
            .add_role(role)
            .expect("Unable to add role");

        let stored_role = role_based_auth_store
            .get_role("test-role")
            .expect("Unable to lookup role by id")
            .expect("Did not find the added role");

        assert_eq!("test-role", stored_role.id());
        assert_eq!("Test Role", stored_role.display_name());
        assert_eq!(
            &["a".to_string(), "b".to_string(), "c".to_string()],
            stored_role.permissions()
        );
    }

    /// This tests verifies the following:
    /// 1. Adds two roles via the store API
    /// 2. Verifies they have been added by listing the roles via the store API
    #[test]
    fn sqlite_list_roles() {
        let pool = create_connection_pool_and_migrate();

        let role_based_auth_store = DieselRoleBasedAuthorizationStore::new(pool);

        let role = RoleBuilder::new()
            .with_id("test-role-1".into())
            .with_display_name("Test Role 1".into())
            .with_permissions(vec!["a".to_string(), "b".to_string(), "c".to_string()])
            .build()
            .expect("Unable to build role");

        role_based_auth_store
            .add_role(role)
            .expect("Unable to add role");

        let role = RoleBuilder::new()
            .with_id("test-role-2".into())
            .with_display_name("Test Role 2".into())
            .with_permissions(vec!["x".to_string(), "y".to_string(), "z".to_string()])
            .build()
            .expect("Unable to build role");

        role_based_auth_store
            .add_role(role)
            .expect("Unable to add role");

        let mut stored_role_iter = role_based_auth_store
            .list_roles()
            .expect("Unable to lookup role by id");

        assert_eq!(2, stored_role_iter.len());

        let stored_role = stored_role_iter
            .next()
            .expect("has 2 items, but returned None");
        assert_eq!("test-role-1", stored_role.id());
        assert_eq!("Test Role 1", stored_role.display_name());
        assert_eq!(
            &["a".to_string(), "b".to_string(), "c".to_string()],
            stored_role.permissions()
        );

        let stored_role = stored_role_iter
            .next()
            .expect("has 2 items, but returned None");
        assert_eq!("test-role-2", stored_role.id());
        assert_eq!("Test Role 2", stored_role.display_name());
        assert_eq!(
            &["x".to_string(), "y".to_string(), "z".to_string()],
            stored_role.permissions()
        );
    }

    /// This tests verifies the following:
    /// 1. Adds a role and verifies that it has been inserted
    /// 2. Update the role and verifies that it has been changed, via the store API
    #[test]
    fn sqlite_update_role() {
        let pool = create_connection_pool_and_migrate();

        let role_based_auth_store = DieselRoleBasedAuthorizationStore::new(pool);

        let role = RoleBuilder::new()
            .with_id("test-role".into())
            .with_display_name("Test Role".into())
            .with_permissions(vec!["a".to_string(), "b".to_string(), "c".to_string()])
            .build()
            .expect("Unable to build role");

        role_based_auth_store
            .add_role(role)
            .expect("Unable to add role");

        let stored_role = role_based_auth_store
            .get_role("test-role")
            .expect("Unable to lookup role by id")
            .expect("Did not find the added role");

        assert_eq!("test-role", stored_role.id());
        assert_eq!("Test Role", stored_role.display_name());
        assert_eq!(
            &["a".to_string(), "b".to_string(), "c".to_string()],
            stored_role.permissions()
        );

        let updated_role = stored_role
            .into_update_builder()
            .with_display_name("Updated Test Role".into())
            .with_permissions(vec!["a".to_string(), "b".to_string()])
            .build()
            .expect("Unable to build updated role");

        role_based_auth_store
            .update_role(updated_role)
            .expect("Unable to update role");

        let stored_role = role_based_auth_store
            .get_role("test-role")
            .expect("Unable to lookup role by id")
            .expect("Did not find the added role");

        assert_eq!("test-role", stored_role.id());
        assert_eq!("Updated Test Role", stored_role.display_name());
        assert_eq!(
            &["a".to_string(), "b".to_string()],
            stored_role.permissions()
        );
    }

    /// This tests verifies the following:
    /// 1. Adds a role and verifies that it has been inserted
    /// 2. Removes a role and verifies that it has been removed, via the store API
    /// 3. Verify that the role permissions have been removed
    #[test]
    fn sqlite_remove_role() {
        let pool = create_connection_pool_and_migrate();

        let role_based_auth_store = DieselRoleBasedAuthorizationStore::new(pool.clone());

        let role = RoleBuilder::new()
            .with_id("test-role".into())
            .with_display_name("Test Role".into())
            .with_permissions(vec!["a".to_string(), "b".to_string(), "c".to_string()])
            .build()
            .expect("Unable to build role");

        role_based_auth_store
            .add_role(role)
            .expect("Unable to add role");

        let stored_role = role_based_auth_store
            .get_role("test-role")
            .expect("Unable to lookup role by id")
            .expect("Did not find the added role");

        assert_eq!("test-role", stored_role.id());
        assert_eq!("Test Role", stored_role.display_name());
        assert_eq!(
            &["a".to_string(), "b".to_string(), "c".to_string()],
            stored_role.permissions()
        );

        role_based_auth_store
            .remove_role(stored_role.id())
            .expect("Unable to remove role");

        let stored_role = role_based_auth_store
            .get_role("test-role")
            .expect("Unable to lookup role by id");
        assert!(stored_role.is_none());

        // verify that the permissions have been removed (in a block, so the connection is dropped)
        {
            let connection = pool.get().expect("Unable to get connection");
            let perms = schema::role_permissions::table
                .filter(schema::role_permissions::role_id.eq("test-role"))
                .load::<models::RolePermissionModel>(&*connection)
                .expect("Unable to load permissions");
            assert!(perms.is_empty());
        }

        // verify that the remove is idempotent
        role_based_auth_store
            .remove_role("test-role")
            .expect("Unable to remove role");
    }

    /// This test verifies the following:
    /// 1. Adds a role.
    /// 2. Adds an assignment for that role
    /// 3. Verifies the assignment was added via the store API
    #[test]
    fn sqlite_add_and_get_assignment() {
        let pool = create_connection_pool_and_migrate();

        let role_based_auth_store = DieselRoleBasedAuthorizationStore::new(pool.clone());

        let role = RoleBuilder::new()
            .with_id("test-role".into())
            .with_display_name("Test Role".into())
            .with_permissions(vec!["a".to_string(), "b".to_string(), "c".to_string()])
            .build()
            .expect("Unable to build role");

        role_based_auth_store
            .add_role(role)
            .expect("Unable to add role");

        let assignment = AssignmentBuilder::new()
            .with_identity(Identity::User("some-user-id".into()))
            .with_roles(vec!["test-role".to_string()])
            .build()
            .expect("Unable to build assignment");

        role_based_auth_store
            .add_assignment(assignment)
            .expect("Unable to add assignment");

        let stored_assignment = role_based_auth_store
            .get_assignment(&Identity::User("some-user-id".into()))
            .expect("Unable to get assignment")
            .expect("Assignment was not found");

        assert_eq!(
            &Identity::User("some-user-id".into()),
            stored_assignment.identity()
        );
        assert_eq!(&vec!["test-role".to_string()], stored_assignment.roles());
    }

    /// This test verifies the following:
    /// 1. Adds a role.
    /// 2. Add two assignments for that role
    /// 3. Verifies the assignments were added via the store's list API
    #[test]
    fn sqlite_list_assignments() {
        let pool = create_connection_pool_and_migrate();

        let role_based_auth_store = DieselRoleBasedAuthorizationStore::new(pool.clone());

        let role = RoleBuilder::new()
            .with_id("test-role".into())
            .with_display_name("Test Role".into())
            .with_permissions(vec!["a".to_string(), "b".to_string(), "c".to_string()])
            .build()
            .expect("Unable to build role");

        role_based_auth_store
            .add_role(role)
            .expect("Unable to add role");

        let assignment = AssignmentBuilder::new()
            .with_identity(Identity::User("some-user-id-1".into()))
            .with_roles(vec!["test-role".to_string()])
            .build()
            .expect("Unable to build assignment");

        role_based_auth_store
            .add_assignment(assignment)
            .expect("Unable to add assignment");

        let assignment = AssignmentBuilder::new()
            .with_identity(Identity::Key("some-key-1".into()))
            .with_roles(vec!["test-role".to_string()])
            .build()
            .expect("Unable to build assignment");

        role_based_auth_store
            .add_assignment(assignment)
            .expect("Unable to add assignment");

        let mut stored_assignment_iter = role_based_auth_store
            .list_assignments()
            .expect("Unable to get assignment");

        assert_eq!(2, stored_assignment_iter.len());

        let stored_assignment = stored_assignment_iter
            .next()
            .expect("has 2 items, but returned None");
        assert_eq!(
            &Identity::User("some-user-id-1".into()),
            stored_assignment.identity()
        );
        assert_eq!(&vec!["test-role".to_string()], stored_assignment.roles());

        let stored_assignment = stored_assignment_iter
            .next()
            .expect("has 2 items, but returned None");
        assert_eq!(
            &Identity::Key("some-key-1".into()),
            stored_assignment.identity()
        );
        assert_eq!(&vec!["test-role".to_string()], stored_assignment.roles());
    }

    /// This test verifies the following:
    /// 1. Add two roles
    /// 2. Add an assignment to one of the roles
    /// 3. Update the assignment to have both roles and verify via the store API
    /// 4. Update the assignment to only have the other role, and verify via the store API
    #[test]
    fn sqlite_update_assignment() {
        let pool = create_connection_pool_and_migrate();

        let role_based_auth_store = DieselRoleBasedAuthorizationStore::new(pool.clone());

        let role = RoleBuilder::new()
            .with_id("test-role-1".into())
            .with_display_name("Test Role 1".into())
            .with_permissions(vec!["a".to_string(), "b".to_string(), "c".to_string()])
            .build()
            .expect("Unable to build role");

        role_based_auth_store
            .add_role(role)
            .expect("Unable to add role");

        let role = RoleBuilder::new()
            .with_id("test-role-2".into())
            .with_display_name("Test Role 2".into())
            .with_permissions(vec!["x".to_string(), "y".to_string(), "z".to_string()])
            .build()
            .expect("Unable to build role");

        role_based_auth_store
            .add_role(role)
            .expect("Unable to add role");

        let assignment = AssignmentBuilder::new()
            .with_identity(Identity::User("some-user-id".into()))
            .with_roles(vec!["test-role-1".to_string()])
            .build()
            .expect("Unable to build assignment");

        role_based_auth_store
            .add_assignment(assignment)
            .expect("Unable to add assignment");

        let stored_assignment = role_based_auth_store
            .get_assignment(&Identity::User("some-user-id".into()))
            .expect("Unable to get assignment")
            .expect("Assignment was not found");

        assert_eq!(
            &Identity::User("some-user-id".into()),
            stored_assignment.identity()
        );
        assert_eq!(&vec!["test-role-1".to_string()], stored_assignment.roles());

        let updated_assignment = stored_assignment
            .into_update_builder()
            .with_roles(vec!["test-role-1".to_string(), "test-role-2".to_string()])
            .build()
            .expect("Unable to build updated assignment");

        role_based_auth_store
            .update_assignment(updated_assignment)
            .expect("Unable to update assignment");

        let stored_assignment = role_based_auth_store
            .get_assignment(&Identity::User("some-user-id".into()))
            .expect("Unable to get assignment")
            .expect("Assignment was not found");

        assert_eq!(
            &Identity::User("some-user-id".into()),
            stored_assignment.identity()
        );
        assert_eq!(
            &vec!["test-role-1".to_string(), "test-role-2".to_string()],
            stored_assignment.roles()
        );

        let updated_assignment = stored_assignment
            .into_update_builder()
            .with_roles(vec!["test-role-2".to_string()])
            .build()
            .expect("Unable to build updated assignment");

        role_based_auth_store
            .update_assignment(updated_assignment)
            .expect("Unable to update assignment");

        let stored_assignment = role_based_auth_store
            .get_assignment(&Identity::User("some-user-id".into()))
            .expect("Unable to get assignment")
            .expect("Assignment was not found");

        assert_eq!(
            &Identity::User("some-user-id".into()),
            stored_assignment.identity()
        );
        assert_eq!(&vec!["test-role-2".to_string()], stored_assignment.roles());
    }

    /// This test verifies the following:
    /// 1. Add a role
    /// 2. Add an assignment for the role and verify with the store API
    /// 3. Remove the assignment and verify its removal with the API
    /// 4. Verify that the assignment records have been removed.
    /// 5. Verify that the removal is idempotent
    #[test]
    fn sqlite_remove_assignment() {
        let pool = create_connection_pool_and_migrate();

        let role_based_auth_store = DieselRoleBasedAuthorizationStore::new(pool.clone());

        let role = RoleBuilder::new()
            .with_id("test-role".into())
            .with_display_name("Test Role".into())
            .with_permissions(vec!["a".to_string(), "b".to_string(), "c".to_string()])
            .build()
            .expect("Unable to build role");

        role_based_auth_store
            .add_role(role)
            .expect("Unable to add role");

        let assignment = AssignmentBuilder::new()
            .with_identity(Identity::User("some-user-id".into()))
            .with_roles(vec!["test-role".to_string()])
            .build()
            .expect("Unable to build assignment");

        role_based_auth_store
            .add_assignment(assignment)
            .expect("Unable to add assignment");

        let stored_assignment = role_based_auth_store
            .get_assignment(&Identity::User("some-user-id".into()))
            .expect("Unable to get assignment")
            .expect("Assignment was not found");

        assert_eq!(
            &Identity::User("some-user-id".into()),
            stored_assignment.identity()
        );
        assert_eq!(&vec!["test-role".to_string()], stored_assignment.roles());

        role_based_auth_store
            .remove_assignment(&Identity::User("some-user-id".into()))
            .expect("Unable to remove assignment");

        let stored_assignment = role_based_auth_store
            .get_assignment(&Identity::User("some-user-id".into()))
            .expect("Unable to get assignment");

        assert!(stored_assignment.is_none());

        // verify that the assignments have been removed (in a block, so the connection is dropped)
        {
            let connection = pool.get().expect("Unable to get connection");
            let perms = schema::assignments::table
                .filter(schema::assignments::identity.eq("some-user-id"))
                .load::<models::RolePermissionModel>(&*connection)
                .expect("Unable to load permissions");
            assert!(perms.is_empty());
        }

        // verify that the removal is idempotent
        role_based_auth_store
            .remove_assignment(&Identity::User("some-user-id".into()))
            .expect("Unable to remove assignment");
    }

    /// Creates a connection pool for an in-memory SQLite database with only a single connection
    /// available. Each connection is backed by a different in-memory SQLite database, so limiting
    /// the pool to a single connection insures that the same DB is used for all operations.
    fn create_connection_pool_and_migrate() -> Pool<ConnectionManager<SqliteConnection>> {
        let connection_manager = ConnectionManager::<SqliteConnection>::new(":memory:");
        let pool = Pool::builder()
            .max_size(1)
            .build(connection_manager)
            .expect("Failed to build connection pool");

        run_sqlite_migrations(&*pool.get().expect("Failed to get connection for migrations"))
            .expect("Failed to run migrations");

        pool
    }
}
