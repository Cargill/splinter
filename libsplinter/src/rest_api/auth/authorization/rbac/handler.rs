// Copyright 2018-2021 Cargill Incorporated
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

use crate::error::InternalError;

use crate::rest_api::auth::{
    authorization::{AuthorizationHandler, AuthorizationHandlerResult},
    identity::Identity,
};

use super::store::{RoleBasedAuthorizationStore, ADMIN_ROLE_ID};

/// A Role-based authorization handler.
///
/// This handler determines if an identity has a requested permission by examining the roles that
/// it has been assigned.  If one of the identity's assigned roles contains the permission, then
/// the identity is allowed access. If not, the handler defers to the next handler in the chain.
///
/// It currently does not deny any permissions.
pub struct RoleBasedAuthorizationHandler {
    role_based_auth_store: Box<dyn RoleBasedAuthorizationStore>,
}

impl RoleBasedAuthorizationHandler {
    /// Construct a new role-based authorization handler with the given store.
    pub fn new(role_based_auth_store: Box<dyn RoleBasedAuthorizationStore>) -> Self {
        Self {
            role_based_auth_store,
        }
    }
}

impl AuthorizationHandler for RoleBasedAuthorizationHandler {
    fn has_permission(
        &self,
        identity: &Identity,
        permission_id: &str,
    ) -> Result<AuthorizationHandlerResult, InternalError> {
        match identity.into() {
            Some(identity) => Ok(self
                .role_based_auth_store
                .get_assigned_roles(&identity)
                .map_err(|err| InternalError::from_source(Box::new(err)))?
                .find(|role| {
                    role.id() == ADMIN_ROLE_ID
                        || role.permissions().iter().any(|perm| perm == permission_id)
                })
                .map(|_| AuthorizationHandlerResult::Allow)
                .unwrap_or(AuthorizationHandlerResult::Continue)),
            None => Ok(AuthorizationHandlerResult::Continue),
        }
    }

    fn clone_box(&self) -> Box<dyn AuthorizationHandler> {
        Box::new(RoleBasedAuthorizationHandler {
            role_based_auth_store: self.role_based_auth_store.clone_box(),
        })
    }
}

#[cfg(all(test, feature = "sqlite",))]
mod tests {
    use super::*;

    use crate::rest_api::auth::authorization::rbac::store::{
        AssignmentBuilder, DieselRoleBasedAuthorizationStore, Identity as StoreIdentity,
        RoleBuilder,
    };

    use crate::migrations::run_sqlite_migrations;

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };

    #[test]
    fn allow_key_identity_with_assignment() {
        test_allow_identity_with_assignment(
            Identity::Key("abc123".into()),
            StoreIdentity::Key("abc123".into()),
        );
    }

    #[test]
    fn allow_user_identity_with_assignment() {
        test_allow_identity_with_assignment(
            Identity::User("some-user-id".into()),
            StoreIdentity::User("some-user-id".into()),
        );
    }

    #[test]
    fn allow_key_identity_admin() {
        test_allow_identity_admin(
            Identity::Key("abc123".into()),
            StoreIdentity::Key("abc123".into()),
        );
    }

    #[test]
    fn allow_user_identity_admin() {
        test_allow_identity_admin(
            Identity::User("some-user-id".into()),
            StoreIdentity::User("some-user-id".into()),
        );
    }

    #[test]
    fn continue_key_identity_with_assignment_mismatch() {
        test_continue_identity_with_mismatched_assignment(
            Identity::Key("abc123".into()),
            StoreIdentity::Key("abc123".into()),
        );
    }

    #[test]
    fn continue_user_identity_with_assignment_mismatch() {
        test_continue_identity_with_mismatched_assignment(
            Identity::User("some-user-id".into()),
            StoreIdentity::User("some-user-id".into()),
        );
    }

    #[test]
    fn continue_key_identity_with_no_assignment() {
        test_continue_identity_with_no_assignment(Identity::Key("abc123".into()));
    }

    #[test]
    fn continue_user_identity_with_no_assignment() {
        test_continue_identity_with_no_assignment(Identity::User("some-user-id".into()));
    }

    #[test]
    fn continue_custom_identity() {
        let role_based_auth_store = create_role_based_authorization_store();
        let handler = RoleBasedAuthorizationHandler::new(role_based_auth_store);
        let result = handler
            .has_permission(&Identity::Custom("Anything".into()), "a")
            .expect("Should have returned an auth result");

        assert!(matches!(result, AuthorizationHandlerResult::Continue));
    }

    /// This test checks that an identity with an assigned role will return Allow when queried.
    fn test_allow_identity_with_assignment(identity: Identity, store_identity: StoreIdentity) {
        let role_based_auth_store = create_role_based_authorization_store();

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

        let assignment = AssignmentBuilder::new()
            .with_identity(store_identity)
            .with_roles(vec!["test-role-1".to_string(), "test-role-2".to_string()])
            .build()
            .expect("Unable to build assignment");

        role_based_auth_store
            .add_assignment(assignment)
            .expect("Unable to add assignment");

        role_based_auth_store
            .add_role(role)
            .expect("Unable to add role");

        let handler = RoleBasedAuthorizationHandler::new(role_based_auth_store);

        // Check a permission in the first role
        let result = handler
            .has_permission(&identity, "a")
            .expect("Should have returned an auth result");

        assert!(matches!(result, AuthorizationHandlerResult::Allow));

        // Check a permission in the second role
        let result = handler
            .has_permission(&identity, "z")
            .expect("Should have returned an auth result");

        assert!(matches!(result, AuthorizationHandlerResult::Allow));
    }

    /// This test checks that an identity that is assigned the admin role will return Allow when
    /// queried.
    fn test_allow_identity_admin(identity: Identity, store_identity: StoreIdentity) {
        let role_based_auth_store = create_role_based_authorization_store();

        let assignment = AssignmentBuilder::new()
            .with_identity(store_identity)
            .with_roles(vec![ADMIN_ROLE_ID.to_string()])
            .build()
            .expect("Unable to build assignment");

        role_based_auth_store
            .add_assignment(assignment)
            .expect("Unable to add assignment");

        let handler = RoleBasedAuthorizationHandler::new(role_based_auth_store);

        // Check an arbitrary permission
        let result = handler
            .has_permission(&identity, "perm")
            .expect("Should have returned an auth result");

        assert!(matches!(result, AuthorizationHandlerResult::Allow));
    }

    /// This test checks that an identity with an assignment that does not include the permission
    /// being checked returns Continue.
    fn test_continue_identity_with_mismatched_assignment(
        identity: Identity,
        store_identity: StoreIdentity,
    ) {
        let role_based_auth_store = create_role_based_authorization_store();

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

        let assignment = AssignmentBuilder::new()
            .with_identity(store_identity)
            .with_roles(vec!["test-role-1".to_string(), "test-role-2".to_string()])
            .build()
            .expect("Unable to build assignment");

        role_based_auth_store
            .add_assignment(assignment)
            .expect("Unable to add assignment");

        role_based_auth_store
            .add_role(role)
            .expect("Unable to add role");

        let handler = RoleBasedAuthorizationHandler::new(role_based_auth_store);
        let result = handler
            .has_permission(&identity, "non-assigned-permission")
            .expect("Should have returned an auth result");

        assert!(matches!(result, AuthorizationHandlerResult::Continue));
    }

    fn test_continue_identity_with_no_assignment(identity: Identity) {
        let role_based_auth_store = create_role_based_authorization_store();

        let handler = RoleBasedAuthorizationHandler::new(role_based_auth_store);
        let result = handler
            .has_permission(&identity, "non-assigned-permission")
            .expect("Should have returned an auth result");

        assert!(matches!(result, AuthorizationHandlerResult::Continue));
    }

    /// Creates a RoleBasedAuthorizationStore
    fn create_role_based_authorization_store() -> Box<dyn RoleBasedAuthorizationStore> {
        let pool = create_connection_pool_and_migrate();
        Box::new(DieselRoleBasedAuthorizationStore::new(pool))
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
