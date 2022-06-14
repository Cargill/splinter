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

#[cfg(feature = "rest-api-actix-web-1")]
use crate::service::instance::EndpointFactory;
use crate::service::instance::{FactoryCreateError, ServiceFactory};

use super::OrchestratableService;

#[cfg(feature = "rest-api-actix-web-1")]
/// A service factory that produces orchestratable services.
pub trait OrchestratableServiceFactory: ServiceFactory + EndpointFactory {
    /// Create a Service instance with the given ID, of the given type, the given circuit_id,
    /// with the given arguments.
    fn create_orchestratable_service(
        &self,
        service_id: String,
        service_type: &str,
        circuit_id: &str,
        args: HashMap<String, String>,
    ) -> Result<Box<dyn OrchestratableService>, FactoryCreateError>;
}

#[cfg(not(feature = "rest-api-actix-web-1"))]
/// A service factory that produces orchestratable services.
pub trait OrchestratableServiceFactory: ServiceFactory {
    /// Create a Service instance with the given ID, of the given type, the given circuit_id,
    /// with the given arguments.
    fn create_orchestratable_service(
        &self,
        service_id: String,
        service_type: &str,
        circuit_id: &str,
        args: HashMap<String, String>,
    ) -> Result<Box<dyn OrchestratableService>, FactoryCreateError>;
}
