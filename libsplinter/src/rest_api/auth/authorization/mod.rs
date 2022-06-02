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

//! Tools for determining client/user authorization

#[cfg(feature = "authorization-handler-allow-keys")]
pub mod allow_keys;
#[cfg(feature = "authorization-handler-maintenance")]
pub mod maintenance;
mod permission;
mod permission_map;
#[cfg(feature = "authorization-handler-rbac")]
pub mod rbac;
pub(in crate::rest_api) mod routes;

pub use permission::Permission;
pub use permission_map::PermissionMap;

use crate::error::InternalError;

use super::identity::Identity;

/// An authorization handler's decision about whether to allow, deny, or pass on the request
pub enum AuthorizationHandlerResult {
    /// The authorization handler has granted the requested permission
    Allow,
    /// The authorization handler has denied the requested permission
    Deny,
    /// The authorization handler is not able to determine if the requested permission should be
    /// granted or denied
    Continue,
}

/// Determines if a client has some permissions
pub trait AuthorizationHandler: Send + Sync {
    /// Determines if the given identity has the requested permission
    fn has_permission(
        &self,
        identity: &Identity,
        permission_id: &str,
    ) -> Result<AuthorizationHandlerResult, InternalError>;

    /// Clone implementation for `AuthorizationHandler`. The implementation of the `Clone` trait for
    /// `Box<dyn AuthorizationHandler>` calls this method.
    fn clone_box(&self) -> Box<dyn AuthorizationHandler>;
}

impl Clone for Box<dyn AuthorizationHandler> {
    fn clone(&self) -> Box<dyn AuthorizationHandler> {
        self.clone_box()
    }
}
