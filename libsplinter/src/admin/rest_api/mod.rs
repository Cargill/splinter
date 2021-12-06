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

//! This module defines the REST API endpoints for interacting with the Splinter admin service.

#[cfg(feature = "rest-api-actix")]
mod actix;
#[cfg(feature = "rest-api-actix")]
mod error;
mod resources;

use crate::admin::service::AdminService;
use crate::circuit::store;
use crate::rest_api::{Resource, RestResourceProvider};

/// The admin service provides the following endpoints as REST API resources:
///
/// * `GET /ws/admin/register/{type}` - Register as an application authorization handler for the
///   given circuit management type
/// * `POST /admin/submit` - Submit a circuit management payload
/// * `GET /admin/proposals` - List circuit proposals in Splinter's state
/// * `GET /admin/proposals/{circuit_id}` - Fetch a specific circuit proposal in Splinter's state
///   by circuit ID
///
/// These endpoints are only available if the following REST API backend feature is enabled:
///
/// * `rest-api-actix`
impl RestResourceProvider for AdminService {
    fn resources(&self) -> Vec<Resource> {
        // Allowing unused_mut because resources must be mutable if feature rest-api-actix is
        // enabled
        #[allow(unused_mut)]
        let mut resources = Vec::new();

        #[cfg(feature = "rest-api-actix")]
        {
            resources.append(&mut vec![
                actix::ws_register_type::make_application_handler_registration_route(
                    self.commands(),
                ),
                actix::submit::make_submit_route(self.commands()),
                actix::proposals_circuit_id::make_fetch_proposal_resource(self.proposals()),
                actix::proposals::make_list_proposals_resource(self.proposals()),
            ]);
        }

        resources
    }
}

/// Provides the REST API [Resource](splinter::rest_api::Resource) definitions for
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
/// * `rest-api-actix`
#[derive(Clone)]
// Allow dead code here as this is a part of the public API but it may never be used
#[allow(dead_code)]
pub struct CircuitResourceProvider<T: store::CircuitStore> {
    node_id: String,
    store: T,
}

impl<T: store::CircuitStore + 'static> CircuitResourceProvider<T> {
    pub fn new(node_id: String, store: T) -> Self {
        Self { node_id, store }
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
/// * `rest-api-actix`
impl<T: store::CircuitStore + 'static> RestResourceProvider for CircuitResourceProvider<T> {
    fn resources(&self) -> Vec<Resource> {
        // Allowing unused_mut because resources must be mutable if feature rest-api-actix is
        // enabled
        #[allow(unused_mut)]
        let mut resources = Vec::new();

        #[cfg(feature = "rest-api-actix")]
        {
            resources.append(&mut vec![
                actix::circuits_circuit_id::make_fetch_circuit_resource(self.store.clone()),
                actix::circuits::make_list_circuits_resource(self.store.clone()),
            ]);
        }

        resources
    }
}
