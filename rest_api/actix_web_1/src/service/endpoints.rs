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

//! Defines REST API functionality for Splinter services.

use std::sync::Arc;

use crate::framework::{Method, RequestGuard};
use actix_web::{web, Error as ActixError, HttpRequest, HttpResponse};
use futures::Future;
#[cfg(feature = "authorization")]
use splinter_rest_api_common::auth::Permission;

use splinter::service::instance::ServiceInstance;

/// The type for functions that handle REST API requests made to service endpoints.
pub type Handler = Arc<
    dyn Fn(
            HttpRequest,
            web::Payload,
            &dyn ServiceInstance,
        ) -> Box<dyn Future<Item = HttpResponse, Error = ActixError>>
        + Send
        + Sync
        + 'static,
>;

/// Represents a REST API endpoint provided by a service.
pub struct ServiceEndpoint {
    /// The type of service this endpoint belongs to
    pub service_type: String,
    /// The endpoint's route
    pub route: String,
    /// The endpoint's HTTP method
    pub method: Method,
    /// The function that handles requests made to this endpoint
    pub handler: Handler,
    /// Guards for this endpoint
    pub request_guards: Vec<ServiceRequestGuard>,
    #[cfg(feature = "authorization")]
    /// The permission that a client needs to use this endpoint
    pub permission: Permission,
}

impl Clone for ServiceEndpoint {
    fn clone(&self) -> Self {
        let service_type = self.service_type.clone();
        let route = self.route.clone();
        let method = self.method;
        let handler = Arc::clone(&self.handler);
        let request_guards = self.request_guards.clone();
        #[cfg(feature = "authorization")]
        let permission = self.permission;
        Self {
            service_type,
            route,
            method,
            handler,
            request_guards,
            #[cfg(feature = "authorization")]
            permission,
        }
    }
}

// Trait capturing the behaviour of providing a Vec of ServiceEndpoints
pub trait ServiceEndpointProvider {
    fn endpoints(&self) -> Vec<ServiceEndpoint> {
        Vec::new()
    }
}

pub type ServiceRequestGuard = Arc<dyn RequestGuard + 'static>;
