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
    Assignment, Identity, Role, RoleBasedAuthorizationStore, RoleBasedAuthorizationStoreError,
    RoleBuilder,
};

use operations::add_role::RoleBasedAuthorizationStoreAddRole as _;
use operations::get_role::RoleBasedAuthorizationStoreGetRole as _;
use operations::list_roles::RoleBasedAuthorizationStoreListRoles as _;
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
