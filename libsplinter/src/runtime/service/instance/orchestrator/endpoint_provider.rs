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

use crate::service::{
    instance::EndpointFactory,
    rest_api::{ServiceEndpoint, ServiceEndpointProvider},
};

#[derive(Clone)]
pub struct OrchestratorEndpointFactory {
    provider: OrchestratableServiceEndpointProvider,
}

#[derive(Default)]
pub struct OrchestratorEndpointFactoryBuilder {
    providers: Vec<Box<dyn ServiceEndpointProvider>>,
}

impl OrchestratorEndpointFactoryBuilder {
    pub fn with_provider(mut self, provider: Box<dyn ServiceEndpointProvider>) -> Self {
        self.providers.push(provider);
        self
    }

    pub fn build(self) -> OrchestratorEndpointFactory {
        let endpoints: Vec<ServiceEndpoint> = self
            .providers
            .iter()
            .flat_map(|provider| provider.endpoints())
            .collect();
        let provider = OrchestratableServiceEndpointProvider { endpoints };
        OrchestratorEndpointFactory { provider }
    }
}

impl EndpointFactory for OrchestratorEndpointFactory {
    fn get_rest_endpoint_provider(
        &self,
    ) -> Box<dyn crate::service::rest_api::ServiceEndpointProvider> {
        Box::new(self.provider.clone())
    }
}

#[derive(Clone)]
pub struct OrchestratableServiceEndpointProvider {
    endpoints: Vec<ServiceEndpoint>,
}

impl ServiceEndpointProvider for OrchestratableServiceEndpointProvider {
    fn endpoints(&self) -> Vec<crate::service::rest_api::ServiceEndpoint> {
        self.endpoints.clone()
    }
}
