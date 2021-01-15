// Copyright 2018-2021 Cargill Incorporated
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

use super::{FactoryCreateError, Service};

/// A `ServiceFactory` creates services.
pub trait ServiceFactory: Send {
    /// Return the available service types that this factory can create.
    fn available_service_types(&self) -> &[String];

    /// Create a Service instance with the given ID, of the given type, the given circuit_id,
    /// with the given arguments.
    fn create(
        &self,
        service_id: String,
        service_type: &str,
        circuit_id: &str,
        args: HashMap<String, String>,
    ) -> Result<Box<dyn Service>, FactoryCreateError>;

    #[cfg(feature = "rest-api")]
    /// Get the [`ServiceEndpoint`] definitions that represent the REST API resources provided by
    /// the services that this factory can create.
    ///
    /// [`ServiceEndpoint`]: rest_api/struct.ServiceEndpoint.html
    fn get_rest_endpoints(&self) -> Vec<super::rest_api::ServiceEndpoint>;
}
