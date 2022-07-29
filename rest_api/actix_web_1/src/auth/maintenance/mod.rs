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

//! REST API endpoints for the maintenance mode authorization handler

mod actix;

use crate::framework::{Resource, RestResourceProvider};

use splinter_rest_api_common::auth::MaintenanceModeAuthorizationHandler;
use splinter_rest_api_common::auth::Permission;

pub use self::actix::{make_maintenance_resource, AUTHORIZATION_MAINTENANCE_MIN};

pub const AUTHORIZATION_MAINTENANCE_READ_PERMISSION: Permission = Permission::Check {
    permission_id: "authorization.maintenance.read",
    permission_display_name: "Maintenance mode read",
    permission_description: "Allows the client to check maintenance mode status",
};
pub const AUTHORIZATION_MAINTENANCE_WRITE_PERMISSION: Permission = Permission::Check {
    permission_id: "authorization.maintenance.write",
    permission_display_name: "Maintenance mode write",
    permission_description: "Allows the client to enable/disable maintenance mode",
};

/// The `MaintenanceModeAuthorizationHandler` provides the following endpoints as REST API
/// resources:
///
/// * `GET /authorization/maintenance` - Check if maintenance mode is enabled
/// * `POST /authorization/maintenance` - Enable/disable maintenance mode
///
/// These endpoints are only available if the following REST API backend feature is enabled:
///
/// * `rest-api-actix`
impl RestResourceProvider for MaintenanceModeAuthorizationHandler {
    fn resources(&self) -> Vec<Resource> {
        let resources = vec![actix::make_maintenance_resource(self.clone())];
        resources
    }
}
