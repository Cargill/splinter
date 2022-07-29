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

//! Actix Web 1.x RBAC REST Resource implementations.

mod assignments;
mod roles;

use splinter::rbac::store::RoleBasedAuthorizationStore;

use crate::framework::{Resource, RestResourceProvider};

/// REST Resource Provider for Role-based Authorization REST resources.
pub struct RoleBasedAuthorizationResourceProvider {
    role_based_authorization_store: Box<dyn RoleBasedAuthorizationStore>,
}

impl RoleBasedAuthorizationResourceProvider {
    /// Constructs a new resource provider with the given store.
    pub fn new(role_based_authorization_store: Box<dyn RoleBasedAuthorizationStore>) -> Self {
        Self {
            role_based_authorization_store,
        }
    }
}

impl RestResourceProvider for RoleBasedAuthorizationResourceProvider {
    fn resources(&self) -> Vec<Resource> {
        vec![
            roles::make_roles_resource(self.role_based_authorization_store.clone()),
            roles::make_role_resource(self.role_based_authorization_store.clone()),
            assignments::make_assignments_resource(self.role_based_authorization_store.clone()),
            assignments::make_assignment_resource(self.role_based_authorization_store.clone()),
        ]
    }
}
