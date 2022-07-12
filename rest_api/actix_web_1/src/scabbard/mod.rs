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

pub mod batch_statuses;
pub mod batches;
pub mod state;
pub mod state_address;
pub mod state_root;
pub mod ws_subscribe;

use splinter::service::rest_api::{ServiceEndpoint, ServiceEndpointProvider};

pub struct ScabbardServiceEndpointProvider {
    endpoints: Vec<ServiceEndpoint>,
}

impl ServiceEndpointProvider for ScabbardServiceEndpointProvider {
    fn endpoints(&self) -> Vec<ServiceEndpoint> {
        self.endpoints.clone()
    }
}

impl ScabbardServiceEndpointProvider {
    fn new(endpoints: Vec<ServiceEndpoint>) -> Self {
        Self { endpoints }
    }
}

impl Default for ScabbardServiceEndpointProvider {
    fn default() -> Self {
        let endpoints = vec![
            batches::make_add_batches_to_queue_endpoint(),
            ws_subscribe::make_subscribe_endpoint(),
            batch_statuses::make_get_batch_status_endpoint(),
            state_address::make_get_state_at_address_endpoint(),
            state::make_get_state_with_prefix_endpoint(),
            state_root::make_get_state_root_endpoint(),
        ];
        Self::new(endpoints)
    }
}
