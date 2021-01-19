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

use super::{identity::Identity, AuthorizationHandler, AuthorizationHandlerResult};

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
}

impl MaintenanceModeAuthorizationHandler {
    /// Constructs a new `MaintenanceModeAuthorizationHandler`
    pub fn new() -> Self {
        Self::default()
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
        _identity: &Identity,
        permission_id: &str,
    ) -> Result<AuthorizationHandlerResult, InternalError> {
        if !permission_id.ends_with(".read") && self.maintenance_mode.load(Ordering::Relaxed) {
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
        let handler = MaintenanceModeAuthorizationHandler::new();
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
        let handler = MaintenanceModeAuthorizationHandler::new();
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
}
