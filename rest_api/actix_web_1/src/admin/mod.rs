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

mod circuits;
mod circuits_circuit_id;
mod error;
mod proposals;
mod proposals_circuit_id;
mod resources;
mod submit;
#[cfg(feature = "websocket")]
mod ws_register_type;

use crate::framework::Resource;
use crate::framework::RestResourceProvider;
use splinter::admin::service::AdminService;
use splinter::admin::store::AdminServiceStore;
#[cfg(feature = "authorization")]
use splinter_rest_api_common::auth::Permission;

#[cfg(feature = "authorization")]
const CIRCUIT_READ_PERMISSION: Permission = Permission::Check {
    permission_id: "circuit.read",
    permission_display_name: "Circuit read",
    permission_description: "Allows the client to read circuit state",
};
#[cfg(feature = "authorization")]
const CIRCUIT_WRITE_PERMISSION: Permission = Permission::Check {
    permission_id: "circuit.write",
    permission_display_name: "Circuit write",
    permission_description: "Allows the client to modify circuit state",
};

pub struct AdminServiceRestProvider {
    resources: Vec<Resource>,
}

impl AdminServiceRestProvider {
    pub fn new(source: &AdminService) -> Self {
        let resources = vec![
            #[cfg(feature = "websocket")]
            ws_register_type::make_application_handler_registration_route(source.commands()),
            submit::make_submit_route(source.commands()),
            proposals_circuit_id::make_fetch_proposal_resource(source.proposal_store_factory()),
            proposals::make_list_proposals_resource(source.proposal_store_factory()),
        ];
        Self { resources }
    }
}

impl RestResourceProvider for AdminServiceRestProvider {
    fn resources(&self) -> Vec<Resource> {
        self.resources.clone()
    }
}

/// Provides the REST API [`Resource`](crate::rest_api::Resource) definitions for
/// listing and fetching the circuits in the splinter node's state.
///
/// The following endpoints are provided:
///
/// * `GET /admin/circuits` - List circuits in Splinter's state
/// * `GET /admin/circuits/{circuit_id}` - Fetch a specific circuit in Splinter's state by circuit
///   ID
///
/// These endpoints are only available if the following REST API backend feature is enabled:
///
/// * `rest-api-actix-web-1`
#[derive(Clone)]
pub struct CircuitResourceProvider {
    store: Box<dyn AdminServiceStore>,
}

impl CircuitResourceProvider {
    pub fn new(store: Box<dyn AdminServiceStore>) -> Self {
        Self { store }
    }
}

/// The circuit store provides the following endpoints as REST API resources:
///
/// * `GET /admin/circuits` - List circuits in Splinter's state
/// * `GET /admin/circuits/{circuit_id}` - Fetch a specific circuit in Splinter's state by circuit
///   ID
///
/// These endpoints are only available if the following REST API backend feature is enabled:
///
/// * `rest-api-actix-web-1`
impl RestResourceProvider for CircuitResourceProvider {
    fn resources(&self) -> Vec<Resource> {
        // Allowing unused_mut because resources must be mutable if feature rest-api-actix-web-1 is
        // enabled
        #[allow(unused_mut)]
        let mut resources = Vec::new();

        resources.append(&mut vec![
            circuits_circuit_id::make_fetch_circuit_resource(self.store.clone()),
            circuits::make_list_circuits_resource(self.store.clone()),
        ]);
        resources
    }
}
