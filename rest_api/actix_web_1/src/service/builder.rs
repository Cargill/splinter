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

use std::collections::HashMap;
use std::sync::Mutex;

use actix_web::HttpResponse;
use futures::IntoFuture;
use splinter::error::InternalError;
use splinter::rest_api::Resource;
use splinter::runtime::service::instance::{ManagedService, ServiceDefinition};
use splinter::service::instance::OrchestratableService;
use splinter::{
    runtime::service::instance::ServiceOrchestrator, service::rest_api::ServiceEndpointProvider,
};

use super::ServiceOrchestratorRestResourceProvider;

#[derive(Default)]
pub struct ServiceOrchestratorRestResourceProviderBuilder {
    providers: HashMap<String, Box<dyn ServiceEndpointProvider>>,
}

impl ServiceOrchestratorRestResourceProviderBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_endpoint_factory<S: Into<String>>(
        mut self,
        service_type: S,
        provider: Box<dyn ServiceEndpointProvider>,
    ) -> Self {
        self.providers.insert(service_type.into(), provider);
        self
    }

    pub fn build(
        self,
        orchestrator: &ServiceOrchestrator,
    ) -> ServiceOrchestratorRestResourceProvider {
        let service_ids = orchestrator.list_service_types();
        let resources = service_ids
            .iter()
            .filter_map(|id| self.providers.get(id))
            .fold(vec![], |mut acc, provider| {
                // Get all endpoints for the factory
                let mut resources = provider
                    .endpoints()
                    .into_iter()
                    .map(|endpoint| {
                        let route = format!(
                            "/{}/{{circuit}}/{{service_id}}{}",
                            endpoint.service_type, endpoint.route
                        );
                        let services = orchestrator.services();

                        let mut resource_builder = Resource::build(&route);

                        for request_guard in endpoint.request_guards.into_iter() {
                            resource_builder =
                                resource_builder.add_service_request_guard(request_guard);
                        }

                        let service_type = endpoint.service_type;
                        let handler = endpoint.handler;
                        resource_builder.add_method(
                            endpoint.method,
                            #[cfg(feature = "authorization")]
                            endpoint.permission,
                            move |request, payload| {
                                let circuit = request
                                    .match_info()
                                    .get("circuit")
                                    .unwrap_or("")
                                    .to_string();
                                let service_id = request
                                    .match_info()
                                    .get("service_id")
                                    .unwrap_or("")
                                    .to_string();

                                let service = match lookup_service(
                                    &*services,
                                    &circuit,
                                    &service_id,
                                    &service_type,
                                ) {
                                    Ok(Some(s)) => s,
                                    Ok(None) => {
                                        return Box::new(
                                            HttpResponse::NotFound()
                                                .json(json!({
                                                    "message":
                                                        format!(
                                                            "{} service {} on circuit {} not found",
                                                            service_type, service_id, circuit
                                                        )
                                                }))
                                                .into_future(),
                                        )
                                        .into_future();
                                    }
                                    Err(err) => {
                                        error!("{}", err);
                                        return Box::new(
                                            HttpResponse::InternalServerError()
                                                .json(json!({
                                                    "message": "An internal error occurred"
                                                }))
                                                .into_future(),
                                        )
                                        .into_future();
                                    }
                                };

                                handler(request, payload, service.as_service())
                            },
                        )
                    })
                    .collect::<Vec<_>>();

                acc.append(&mut resources);
                acc
            });
        ServiceOrchestratorRestResourceProvider { resources }
    }
}

fn lookup_service(
    services: &Mutex<HashMap<ServiceDefinition, ManagedService>>,
    circuit: &str,
    service_id: &str,
    service_type: &str,
) -> Result<Option<Box<dyn OrchestratableService>>, InternalError> {
    let services = services.lock().map_err(|_| {
        InternalError::with_message("Orchestrator's service lock is poisoned".into())
    })?;

    Ok(services.iter().find_map(|(service_def, managed_service)| {
        if service_def.service_type == service_type
            && service_def.circuit == circuit
            && service_def.service_id == service_id
        {
            Some(managed_service.service.clone())
        } else {
            None
        }
    }))
}
