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

use actix_web::{Error, HttpResponse};
use futures::{Future, IntoFuture};
#[cfg(feature = "authorization")]
use splinter::rest_api::auth::authorization::Permission;
use splinter_rest_api_common::status::Status;

#[cfg(feature = "authorization")]
pub const STATUS_READ_PERMISSION: Permission = Permission::Check {
    permission_id: "status.read",
    permission_display_name: "Status read",
    permission_description: "Allows the client to get node status info",
};

pub fn get_status(
    node_id: String,
    display_name: String,
    #[cfg(feature = "service-endpoint")] service_endpoint: String,
    network_endpoints: Vec<String>,
    advertised_endpoints: Vec<String>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    let status = Status::new(
        node_id,
        display_name,
        #[cfg(feature = "service-endpoint")]
        service_endpoint,
        network_endpoints,
        advertised_endpoints,
    );

    Box::new(HttpResponse::Ok().json(status).into_future())
}
