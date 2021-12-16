// Copyright 2018-2020 Cargill Incorporated
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

//! This module provides the following endpoints:
//!
//! * `GET /authorization/maintenance` for checking if maintenance mode is enabled
//! * `POST /authorization/maintenance` for enabling/disabling maintenance mode

use actix_web::{web, Error, HttpRequest, HttpResponse};
use futures::{future::IntoFuture, Future};

use crate::rest_api::{
    actix_web_1::{Method, ProtocolVersionRangeGuard, Resource},
    auth::authorization::maintenance::MaintenanceModeAuthorizationHandler,
    ErrorResponse, SPLINTER_PROTOCOL_VERSION,
};

use super::{
    resources::PostMaintenanceModeQuery, AUTHORIZATION_MAINTENANCE_READ_PERMISSION,
    AUTHORIZATION_MAINTENANCE_WRITE_PERMISSION,
};

const AUTHORIZATION_MAINTENANCE_MIN: u32 = 1;

pub fn make_maintenance_resource(auth_handler: MaintenanceModeAuthorizationHandler) -> Resource {
    let auth_handler1 = auth_handler.clone();
    Resource::build("/authorization/maintenance")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            AUTHORIZATION_MAINTENANCE_MIN,
            SPLINTER_PROTOCOL_VERSION,
        ))
        .add_method(
            Method::Get,
            AUTHORIZATION_MAINTENANCE_READ_PERMISSION,
            move |_, _| get_maintenance_mode(auth_handler.clone()),
        )
        .add_method(
            Method::Post,
            AUTHORIZATION_MAINTENANCE_WRITE_PERMISSION,
            move |r, _| post_maintenance_mode(r, auth_handler1.clone()),
        )
}

fn get_maintenance_mode(
    auth_handler: MaintenanceModeAuthorizationHandler,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    Box::new(
        HttpResponse::Ok()
            .body(auth_handler.is_maintenance_mode_enabled().to_string())
            .into_future(),
    )
}

fn post_maintenance_mode(
    req: HttpRequest,
    auth_handler: MaintenanceModeAuthorizationHandler,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    Box::new(
        match web::Query::<PostMaintenanceModeQuery>::from_query(req.query_string()) {
            Ok(query) => {
                auth_handler.set_maintenance_mode(query.enabled);
                HttpResponse::Ok().finish().into_future()
            }
            _ => HttpResponse::BadRequest()
                .json(ErrorResponse::bad_request("Invalid query"))
                .into_future(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use reqwest::{blocking::Client, StatusCode, Url};

    use crate::rest_api::actix_web_1::{RestApiBuilder, RestApiShutdownHandle};

    /// Verifies the `GET` and `POST` methods for the `/authorization/maintenance` resource work
    /// correctly.
    ///
    /// 1. Run the REST API with the maintenance mode endpoints
    /// 2. Verify that `GET /authorization/mainenance` initially returns "false"
    /// 3. Turn maintenance mode on using `POST /authorization/mainenance`
    /// 4. Verify that `GET /authorization/mainenance` now returns "true"
    /// 5. Turn maintenance mode off using `POST /authorization/mainenance`
    /// 6. Verify that `GET /authorization/mainenance` now returns "false"
    #[test]
    fn get_and_post() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_maintenance_resource(
                MaintenanceModeAuthorizationHandler::default(),
            )]);

        let url = Url::parse(&format!("http://{}/authorization/maintenance", bind_url))
            .expect("Failed to parse URL");

        // Check disabled
        let resp = Client::new()
            .get(url.clone())
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .send()
            .expect("Failed to perform request");
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(&resp.text().expect("Failed to get response body"), "false");

        // Enable
        let resp = Client::new()
            .post(url.clone())
            .query(&[("enabled", "true")])
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .send()
            .expect("Failed to perform request");
        assert_eq!(resp.status(), StatusCode::OK);

        // Check enabled
        let resp = Client::new()
            .get(url.clone())
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .send()
            .expect("Failed to perform request");
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(&resp.text().expect("Failed to get response body"), "true");

        // Disable
        let resp = Client::new()
            .post(url.clone())
            .query(&[("enabled", "false")])
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .send()
            .expect("Failed to perform request");
        assert_eq!(resp.status(), StatusCode::OK);

        // Check disabled
        let resp = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .send()
            .expect("Failed to perform request");
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(&resp.text().expect("Failed to get response body"), "false");

        shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    /// Verifies that the `POST /authorization/maintenance` endpoint is idempotent
    ///
    /// 1. Run the REST API with the maintenance mode endpoints
    /// 2. Verify that maintenance mode is initially disabled
    /// 3. Disable maintenance mode using `POST /authorization/mainenance` and verify it's still
    ///    disabled
    /// 4. Enable maintenance mode using `POST /authorization/mainenance` and verify it's enabled
    /// 5. Enable maintenance mode again using `POST /authorization/mainenance` and verify it's
    ///    still enabled
    #[test]
    fn post_idempotent() {
        let handler = MaintenanceModeAuthorizationHandler::default();

        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_maintenance_resource(handler.clone())]);

        assert!(!handler.is_maintenance_mode_enabled());

        let url = Url::parse(&format!("http://{}/authorization/maintenance", bind_url))
            .expect("Failed to parse URL");

        // Disable (idempotent)
        let resp = Client::new()
            .post(url.clone())
            .query(&[("enabled", "false")])
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .send()
            .expect("Failed to perform request");
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(!handler.is_maintenance_mode_enabled());

        // Enable
        let resp = Client::new()
            .post(url.clone())
            .query(&[("enabled", "true")])
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .send()
            .expect("Failed to perform request");
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(handler.is_maintenance_mode_enabled());

        // Enable (idempotent)
        let resp = Client::new()
            .post(url.clone())
            .query(&[("enabled", "true")])
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .send()
            .expect("Failed to perform request");
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(handler.is_maintenance_mode_enabled());

        shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    fn run_rest_api_on_open_port(
        resources: Vec<Resource>,
    ) -> (RestApiShutdownHandle, std::thread::JoinHandle<()>, String) {
        #[cfg(not(feature = "https-bind"))]
        let bind = "127.0.0.1:0";
        #[cfg(feature = "https-bind")]
        let bind = crate::rest_api::BindConfig::Http("127.0.0.1:0".into());

        let result = RestApiBuilder::new()
            .with_bind(bind)
            .add_resources(resources.clone())
            .build_insecure()
            .expect("Failed to build REST API")
            .run_insecure();
        match result {
            Ok((shutdown_handle, join_handle)) => {
                let port = shutdown_handle.port_numbers()[0];
                (shutdown_handle, join_handle, format!("127.0.0.1:{}", port))
            }
            Err(err) => panic!("Failed to run REST API: {}", err),
        }
    }
}
