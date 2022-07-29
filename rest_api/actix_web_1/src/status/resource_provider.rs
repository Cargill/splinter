// Copyright (c) 2019 Target Brands, Inc.
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

use crate::framework::{Resource, RestResourceProvider};

use super::get_status;
#[cfg(feature = "authorization")]
use super::STATUS_READ_PERMISSION;

pub struct StatusResourceProvider {
    resources: Vec<Resource>,
}

impl StatusResourceProvider {
    pub fn new(
        node_id: String,
        display_name: String,
        #[cfg(feature = "service-endpoint")] service_endpoint: String,
        network_endpoints: Vec<String>,
        advertised_endpoints: Vec<String>,
    ) -> Self {
        let handle = move |_, _| {
            get_status(
                node_id.clone(),
                display_name.clone(),
                #[cfg(feature = "service-endpoint")]
                service_endpoint.clone(),
                network_endpoints.clone(),
                advertised_endpoints.clone(),
            )
        };
        #[cfg(feature = "authorization")]
        {
            let status_resource = Resource::build("/status").add_method(
                crate::framework::Method::Get,
                STATUS_READ_PERMISSION,
                handle,
            );
            let resources = vec![status_resource];
            Self { resources }
        }
        #[cfg(not(feature = "authorization"))]
        {
            let status_resource =
                Resource::build("/status").add_method(crate::framework::Method::Get, handle);
            let resources = vec![status_resource];
            Self { resources }
        }
    }
}

impl RestResourceProvider for StatusResourceProvider {
    fn resources(&self) -> Vec<crate::framework::Resource> {
        self.resources.clone()
    }
}
