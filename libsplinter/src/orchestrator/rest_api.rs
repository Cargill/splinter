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

use crate::actix_web::HttpResponse;
use crate::error::InternalError;
use crate::futures::IntoFuture;
use crate::orchestrator::OrchestratableService;
use crate::rest_api::actix_web_1::{Resource, RestResourceProvider};

use super::{ManagedService, ServiceDefinition, ServiceOrchestrator};

/// The `ServiceOrchestrator` exposes REST API resources provided by the
/// [`ServiceFactory::get_rest_endpoints`] methods of its factories. Each factory defines the
/// endpoints provided by the services it creates; the `ServiceOrchestrator` then exposes these
/// endpoints under the `/{service_type}/{circuit}/{service_id}` route.
///
/// [`ServiceFactory::get_rest_endpoints`]:
///   ../service/factory/trait.ServiceFactory.html#tymethod.get_rest_endpoints
impl RestResourceProvider for ServiceOrchestrator {
    fn resources(&self) -> Vec<Resource> {
        // Get endpoints for all factories
        self.service_factories
            .iter()
            .fold(vec![], |mut acc, factory| {
                // Get all endpoints for the factory
                let mut resources = factory
                    .get_rest_endpoints()
                    .into_iter()
                    .map(|endpoint| {
                        let route = format!(
                            "/{}/{{circuit}}/{{service_id}}{}",
                            endpoint.service_type, endpoint.route
                        );
                        let services = self.services.clone();

                        let mut resource_builder = Resource::build(&route);

                        for request_guard in endpoint.request_guards.into_iter() {
                            resource_builder = resource_builder.add_request_guard(request_guard);
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
            })
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
