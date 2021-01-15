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

use splinter::actix_web::{web, Error, HttpRequest, HttpResponse};
use splinter::futures::{Future, IntoFuture};

#[derive(Debug, Serialize, Deserialize)]
struct Status {
    node_id: String,
    display_name: String,
    #[cfg(feature = "service-endpoint")]
    service_endpoint: String,
    network_endpoints: Vec<String>,
    advertised_endpoints: Vec<String>,
    version: String,
}

pub fn get_status(
    node_id: String,
    display_name: String,
    #[cfg(feature = "service-endpoint")] service_endpoint: String,
    network_endpoints: Vec<String>,
    advertised_endpoints: Vec<String>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    let status = Status {
        node_id,
        display_name,
        #[cfg(feature = "service-endpoint")]
        service_endpoint,
        network_endpoints,
        advertised_endpoints,
        version: get_version(),
    };

    Box::new(HttpResponse::Ok().json(status).into_future())
}

pub fn get_openapi(
    _: HttpRequest,
    _: web::Payload,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    Box::new(
        HttpResponse::Ok()
            .body(include_str!("../../api/static/openapi.yml"))
            .into_future(),
    )
}

fn get_version() -> String {
    format!(
        "{}.{}.{}",
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR"),
        env!("CARGO_PKG_VERSION_PATCH")
    )
}
