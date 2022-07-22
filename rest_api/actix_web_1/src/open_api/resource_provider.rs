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

#[cfg(feature = "authorization")]
use splinter::rest_api::auth::authorization::Permission;
use splinter::rest_api::{Method, Resource, RestResourceProvider};

#[derive(Default)]
pub struct OpenApiResourceProvider {}

impl RestResourceProvider for OpenApiResourceProvider {
    fn resources(&self) -> Vec<splinter::rest_api::Resource> {
        #[cfg(feature = "authorization")]
        {
            vec![Resource::build(".openapi.yaml").add_method(
                Method::Get,
                Permission::AllowAuthenticated,
                super::get_openapi,
            )]
        }
        #[cfg(not(feature = "authorization"))]
        {
            vec![Resource::build(".openapi.yaml").add_method(Method::Get, super::get_openapi)]
        }
    }
}
