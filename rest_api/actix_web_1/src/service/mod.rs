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

mod builder;

use splinter::rest_api::actix_web_1::{Resource, RestResourceProvider};

pub use builder::ServiceOrchestratorRestResourceProviderBuilder;

/// The `ServiceOrchestratorRestResourceProvider` exposes REST API resources
/// provided by the [`ServiceFactory::get_rest_endpoints`] methods of the
/// `ServiceOrchestrator` factories. Each factory defines the endpoints provided
/// by the services it creates; the `ServiceOrchestratorRestResourceProvider`
/// then exposes these endpoints under the
/// `/{service_type}/{circuit}/{service_id}` route.
///
/// [`ServiceFactory::get_rest_endpoints`]:
///   ../service/factory/trait.ServiceFactory.html#tymethod.get_rest_endpoints
pub struct ServiceOrchestratorRestResourceProvider {
    resources: Vec<Resource>,
}

impl RestResourceProvider for ServiceOrchestratorRestResourceProvider {
    fn resources(&self) -> Vec<Resource> {
        self.resources.clone()
    }
}
