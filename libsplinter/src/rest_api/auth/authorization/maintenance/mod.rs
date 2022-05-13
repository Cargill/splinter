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

//! An authorization handler that allows write permissions to be temporarily revoked

mod routes;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::error::InternalError;
#[cfg(feature = "authorization-handler-rbac")]
use crate::rest_api::auth::authorization::rbac::store::{
    Identity as RBACIdentity, RoleBasedAuthorizationStore, ADMIN_ROLE_ID,
};
use crate::rest_api::auth::identity::Identity;

use super::{AuthorizationHandler, AuthorizationHandlerResult};

/// An authorization handler that allows write permissions to be temporarily revoked
///
/// For the purposes of this authorization handler, a write permission is any permission whose ID
/// does not end in ".read". Any permission whose ID ends with ".read" will be ignored by this
/// authorization handler (checking those permission will result in
/// [`AuthorizationHandlerResult::Continue`]).
///
/// For all non-read permission checks, this authoirzation handler will decide to deny or pass based
/// on whether or not maintenance mode is enabled. If maintenance mode is enabled, checks for
/// non-read permission will always result in a [`AuthorizationHandlerResult::Deny`] result; if
/// disabled, all permission checks will always result in a [`AuthorizationHandlerResult::Continue`]
/// result.
#[derive(Clone, Default)]
pub struct MaintenanceModeAuthorizationHandler {
    maintenance_mode: Arc<AtomicBool>,
    #[cfg(feature = "authorization-handler-rbac")]
    rbac_store: Option<Box<dyn RoleBasedAuthorizationStore>>,
}

impl MaintenanceModeAuthorizationHandler {
    /// Constructs a new `MaintenanceModeAuthorizationHandler`
    ///
    /// # Arguments
    ///
    /// * `rbac_store` - If provided, this will be used to allow identities with the "admin" role
    ///   defined in the RBAC store to perform write operations even with maintenance mode enabled
    #[cfg(feature = "authorization-handler-rbac")]
    pub fn new(rbac_store: Option<Box<dyn RoleBasedAuthorizationStore>>) -> Self {
        Self {
            rbac_store,
            ..Default::default()
        }
    }

    /// Returns whether or not maintenance mode is enabled
    pub fn is_maintenance_mode_enabled(&self) -> bool {
        self.maintenance_mode.load(Ordering::Relaxed)
    }

    /// Sets whether or not maintenance mode is enabled
    pub fn set_maintenance_mode(&self, maintenance_mode: bool) {
        self.maintenance_mode
            .store(maintenance_mode, Ordering::Relaxed);
    }
}

impl AuthorizationHandler for MaintenanceModeAuthorizationHandler {
    fn has_permission(
        &self,
        // Allow `unused_variables` in case `authorization-handler-rbac` feature is not enabled
        #[allow(unused_variables)] identity: &Identity,
        permission_id: &str,
    ) -> Result<AuthorizationHandlerResult, InternalError> {
        if !permission_id.ends_with(".read") && self.maintenance_mode.load(Ordering::Relaxed) {
            // Check if the client has the "admin" role, in which case they're not denied permission
            #[cfg(feature = "authorization-handler-rbac")]
            {
                let is_admin = self
                    .rbac_store
                    .as_ref()
                    .and_then(|store| {
                        let rbac_identity: Option<RBACIdentity> = identity.into();
                        Some(
                            store
                                .get_assignment(&rbac_identity?)
                                .ok()??
                                .roles()
                                .iter()
                                .any(|role| role == ADMIN_ROLE_ID),
                        )
                    })
                    .unwrap_or(false);
                if is_admin {
                    return Ok(AuthorizationHandlerResult::Continue);
                }
            }
            Ok(AuthorizationHandlerResult::Deny)
        } else {
            Ok(AuthorizationHandlerResult::Continue)
        }
    }

    fn clone_box(&self) -> Box<dyn AuthorizationHandler> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::rest_api::auth::authorization::rbac::store::{
        Assignment, AssignmentBuilder, Role, RoleBasedAuthorizationStore,
        RoleBasedAuthorizationStoreError,
    };

    const ADMIN_USER_IDENTITY: &str = "admin_user";
    const NON_ADMIN_USER_IDENTITY: &str = "non_admin_user";

    /// Verifies that the maintenance mode authorization handler returns a `Continue` for all read
    /// operations, regardless of whether or not maintenance mode is enabled.
    ///
    /// 1. Create a new `MaintenanceModeAuthorizationHandler` and verify that maintenance mode is
    ///    disabled
    /// 2. Call `has_permission` with a read permission and verify that a `Continue` result is
    ///    returned
    /// 3. Enable maintenance mode
    /// 4. Call `has_permission` with a read permission and verify that a `Continue` result is
    ///    returned
    #[test]
    fn auth_handler_read_permissions() {
        let handler = MaintenanceModeAuthorizationHandler::default();
        assert_eq!(handler.is_maintenance_mode_enabled(), false);

        assert!(matches!(
            handler.has_permission(&Identity::Custom("identity".into()), "permission.read"),
            Ok(AuthorizationHandlerResult::Continue)
        ));

        handler.set_maintenance_mode(true);
        assert_eq!(handler.is_maintenance_mode_enabled(), true);

        assert!(matches!(
            handler.has_permission(&Identity::Custom("identity".into()), "permission.read"),
            Ok(AuthorizationHandlerResult::Continue)
        ));
    }

    /// Verifies that the maintenance mode authorization handler returns the correct result for
    /// non-read permissions based on whether or not maintenance mode is enabled.
    ///
    /// 1. Create a new `MaintenanceModeAuthorizationHandler` and verify that a `Continue` result
    ///    is returned by `has_permission` by default
    /// 2. Enable maintenance mode and verify that a `Deny` result is returned by `has_permission`
    /// 3. Disable maintenance mode and verify that a `Continue` result is returned by
    ///    `has_permission` again
    #[test]
    fn auth_handler_non_read_permissions() {
        let handler = MaintenanceModeAuthorizationHandler::default();
        assert_eq!(handler.is_maintenance_mode_enabled(), false);
        assert!(matches!(
            handler.has_permission(&Identity::Custom("identity".into()), "permission"),
            Ok(AuthorizationHandlerResult::Continue)
        ));

        handler.set_maintenance_mode(true);
        assert_eq!(handler.is_maintenance_mode_enabled(), true);
        assert!(matches!(
            handler.has_permission(&Identity::Custom("identity".into()), "permission"),
            Ok(AuthorizationHandlerResult::Deny)
        ));

        handler.set_maintenance_mode(false);
        assert_eq!(handler.is_maintenance_mode_enabled(), false);
        assert!(matches!(
            handler.has_permission(&Identity::Custom("identity".into()), "permission"),
            Ok(AuthorizationHandlerResult::Continue)
        ));
    }

    /// Verifies that the maintenance mode authorization handler returns the correct result for
    /// identities that have been assigned the admin role in the RBAC store.
    ///
    /// 1. Create a new `MaintenanceModeAuthorizationHandler` with a mock RBAC store
    /// 2. Enable maintenance mode
    /// 3. Verify that a `Continue` result is returned by `has_permission` when an identity with the
    ///    admin role is speicified
    /// 4. Verify that a `Deny` result is returned by `has_permission` when an identity without the
    ///    admin role is speicified
    /// 5. Verify that a `Deny` result is returned by `has_permission` when an unknown identity is
    ///    specified
    #[cfg(feature = "authorization-handler-rbac")]
    #[test]
    fn auth_handler_rbac_admin() {
        let handler = MaintenanceModeAuthorizationHandler::new(Some(Box::new(
            MockRoleBasedAuthorizationStore,
        )));

        handler.set_maintenance_mode(true);
        assert_eq!(handler.is_maintenance_mode_enabled(), true);

        assert!(matches!(
            handler.has_permission(&Identity::User(ADMIN_USER_IDENTITY.into()), "permission"),
            Ok(AuthorizationHandlerResult::Continue)
        ));

        assert!(matches!(
            handler.has_permission(
                &Identity::User(NON_ADMIN_USER_IDENTITY.into()),
                "permission"
            ),
            Ok(AuthorizationHandlerResult::Deny)
        ));

        assert!(matches!(
            handler.has_permission(&Identity::User("unknown".into()), "permission"),
            Ok(AuthorizationHandlerResult::Deny)
        ));
    }

    #[derive(Clone)]
    struct MockRoleBasedAuthorizationStore;

    impl RoleBasedAuthorizationStore for MockRoleBasedAuthorizationStore {
        fn get_role(&self, _id: &str) -> Result<Option<Role>, RoleBasedAuthorizationStoreError> {
            unimplemented!()
        }

        fn list_roles(
            &self,
        ) -> Result<Box<dyn ExactSizeIterator<Item = Role>>, RoleBasedAuthorizationStoreError>
        {
            unimplemented!()
        }

        fn add_role(&self, _role: Role) -> Result<(), RoleBasedAuthorizationStoreError> {
            unimplemented!()
        }

        fn update_role(&self, _role: Role) -> Result<(), RoleBasedAuthorizationStoreError> {
            unimplemented!()
        }

        fn remove_role(&self, _role_id: &str) -> Result<(), RoleBasedAuthorizationStoreError> {
            unimplemented!()
        }

        fn get_assignment(
            &self,
            identity: &RBACIdentity,
        ) -> Result<Option<Assignment>, RoleBasedAuthorizationStoreError> {
            let admin_identity = RBACIdentity::User(ADMIN_USER_IDENTITY.into());
            if identity == &admin_identity {
                return Ok(Some(
                    AssignmentBuilder::new()
                        .with_identity(admin_identity)
                        .with_roles(vec![ADMIN_ROLE_ID.into()])
                        .build()?,
                ));
            }

            let non_admin_identity = RBACIdentity::User(NON_ADMIN_USER_IDENTITY.into());
            if identity == &non_admin_identity {
                return Ok(Some(
                    AssignmentBuilder::new()
                        .with_identity(non_admin_identity)
                        .with_roles(vec!["other".into()])
                        .build()?,
                ));
            }

            Ok(None)
        }

        fn get_assigned_roles(
            &self,
            _identity: &RBACIdentity,
        ) -> Result<Box<dyn ExactSizeIterator<Item = Role>>, RoleBasedAuthorizationStoreError>
        {
            unimplemented!()
        }

        fn list_assignments(
            &self,
        ) -> Result<Box<dyn ExactSizeIterator<Item = Assignment>>, RoleBasedAuthorizationStoreError>
        {
            unimplemented!()
        }

        fn add_assignment(
            &self,
            _assignment: Assignment,
        ) -> Result<(), RoleBasedAuthorizationStoreError> {
            unimplemented!()
        }

        fn update_assignment(
            &self,
            _assignment: Assignment,
        ) -> Result<(), RoleBasedAuthorizationStoreError> {
            unimplemented!()
        }

        fn remove_assignment(
            &self,
            _identity: &RBACIdentity,
        ) -> Result<(), RoleBasedAuthorizationStoreError> {
            unimplemented!()
        }

        fn clone_box(&self) -> Box<dyn RoleBasedAuthorizationStore> {
            Box::new(self.clone())
        }
    }
}
