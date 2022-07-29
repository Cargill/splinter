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

use crate::service::rest_api::ServiceEndpointProvider;

pub trait EndpointFactory {
    /// Get the [`ServiceEndpoint`] definitions that represent the REST API resources provided by
    /// the services that this factory can create.
    ///
    /// [`ServiceEndpoint`]: rest_api/struct.ServiceEndpoint.html
    fn get_rest_endpoint_provider(&self) -> Box<dyn ServiceEndpointProvider>;
}
