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

//! This module defines the REST API endpoints for interacting with node registries.

#[cfg(feature = "rest-api-actix")]
mod actix;
mod resources;

use crate::rest_api::{Resource, RestResourceProvider};

use super::RwNodeRegistry;

/// The `RwNodeRegistry` trait service provides the following endpoints as REST API resources:
///
/// * `GET /admin/nodes` - List the nodes in the registry
/// * `POST /admin/nodes` - Add a node to the registry
/// * `GET /admin/nodes/{identity}` - Fetch a specific node in the registry
/// * `PUT /admin/nodes/{identity}` - Replace a node in the registry
/// * `DELETE /admin/nodes/{identity}` - Delete a node from the registry
///
/// These endpoints are only available if the following REST API backend feature is enabled:
///
/// * `rest-api-actix`
impl RestResourceProvider for dyn RwNodeRegistry {
    fn resources(&self) -> Vec<Resource> {
        // Allowing unused_mut because resources must be mutable if feature rest-api-actix is
        // enabled
        #[allow(unused_mut)]
        let mut resources = Vec::new();

        #[cfg(feature = "rest-api-actix")]
        {
            resources.append(&mut vec![
                actix::nodes_identity::make_nodes_identity_resource(self.clone_box()),
                actix::nodes::make_nodes_resource(self.clone_box()),
            ]);
        }

        resources
    }
}
