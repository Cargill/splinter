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

use splinter::service::rest_api::{ServiceEndpoint, ServiceEndpointProvider};

#[cfg(feature = "rest-api-actix-web-1")]
use crate::service::rest_api::actix;

pub struct ScabbardServiceEndpointProvider {}

impl ServiceEndpointProvider for ScabbardServiceEndpointProvider {
    fn endpoints(&self) -> Vec<ServiceEndpoint> {
        #[cfg(feature = "rest-api-actix-web-1")]
        {
            vec![
                actix::batches::make_add_batches_to_queue_endpoint(),
                actix::ws_subscribe::make_subscribe_endpoint(),
                actix::batch_statuses::make_get_batch_status_endpoint(),
                actix::state_address::make_get_state_at_address_endpoint(),
                actix::state::make_get_state_with_prefix_endpoint(),
                actix::state_root::make_get_state_root_endpoint(),
            ]
        }
        #[cfg(not(feature = "rest-api-actix-web-1"))]
        Vec::new()
    }
}
