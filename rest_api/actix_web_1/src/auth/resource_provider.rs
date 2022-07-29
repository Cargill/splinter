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

//! REST API endpoints for authorization tools

use splinter_rest_api_common::auth::Permission;

use crate::auth::make_permission_resource::make_permissions_resource;
use crate::framework::{Resource, RestResourceProvider};

pub const AUTHORIZATION_PERMISSIONS_READ_PERMISSION: Permission = Permission::Check {
    permission_id: "authorization.permissions.read",
    permission_display_name: "Permissions read",
    permission_description: "Allows the client to read REST API permissions",
};

/// Provides the REST API [Resource] definitions for authorization endpoints. The following
/// endpoints are provided:
///
/// * `GET /authorization/permissions` - Get the list of all REST API permissions
///
/// These endpoints are only available if the following REST API backend feature is enabled:
///
/// * `rest-api-actix`
pub struct AuthorizationResourceProvider {
    permissions: Vec<Permission>,
}

impl AuthorizationResourceProvider {
    /// Creates a new `AuthorizationResourceProvider`
    pub fn new(permissions: Vec<Permission>) -> Self {
        Self { permissions }
    }
}

/// `AuthorizationResourceProvider` provides the following endpoints as REST API resources:
///
/// * `GET /authorization/permissions` - Get the list of all REST API permissions
///
/// These endpoints are only available if the following REST API backend feature is enabled:
///
/// * `rest-api-actix`
impl RestResourceProvider for AuthorizationResourceProvider {
    fn resources(&self) -> Vec<Resource> {
        // Allowing unused_mut because resources must be mutable if feature rest-api-actix is
        // enabled
        vec![make_permissions_resource(self.permissions.clone())]
    }
}
