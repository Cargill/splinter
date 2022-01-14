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

//! Actix Web 3 implementation of the admin service REST API endpoints.

use actix_web_3::{web, Resource};

use crate::rest_api::actix_web_3::ResourceProvider;

mod get_admin_circuits;

// An implementation of `ResourceProvider` which returns a list of all the Actix `Resource`s
// related to admin service endpoints.
pub struct AdminResourceProvider {}

impl Default for AdminResourceProvider {
    fn default() -> Self {
        AdminResourceProvider::new()
    }
}

impl AdminResourceProvider {
    pub fn new() -> Self {
        AdminResourceProvider {}
    }
}

impl ResourceProvider for AdminResourceProvider {
    fn resources(&self) -> Vec<Resource> {
        vec![web::resource("/admin/circuits")
            .route(web::get().to(get_admin_circuits::get_admin_circuits))]
    }
}
