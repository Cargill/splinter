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

//! This module provides the `GET /admin/circuits/{circuit_id} endpoint for fetching the
//! definition of a circuit in Splinter's state by its circuit ID.

use actix_web::{error::BlockingError, web, Error, HttpRequest, HttpResponse};
use futures::Future;

use crate::admin::store::AdminServiceStore;
use crate::protocol;
use crate::rest_api::{ErrorResponse, Method, ProtocolVersionRangeGuard, Resource};

use super::super::error::CircuitFetchError;
use super::super::resources;

pub fn make_fetch_circuit_resource(store: Box<dyn AdminServiceStore>) -> Resource {
    Resource::build("/admin/circuits/{circuit_id}")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::ADMIN_FETCH_CIRCUIT_MIN,
            protocol::ADMIN_PROTOCOL_VERSION,
        ))
        .add_method(Method::Get, move |r, _| {
            fetch_circuit(r, web::Data::new(store.clone()))
        })
}

fn fetch_circuit(
    request: HttpRequest,
    store: web::Data<Box<dyn AdminServiceStore>>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    let circuit_id = request
        .match_info()
        .get("circuit_id")
        .unwrap_or("")
        .to_string();

    let protocol_version = match request.headers().get("SplinterProtocolVersion") {
        Some(header_value) => match header_value.to_str() {
            Ok(protocol_version) => Ok(protocol_version.to_string()),
            Err(_) => Err(CircuitFetchError::BadRequest(
                "Unable to get SplinterProtocolVersion".to_string(),
            )),
        },
        None => Ok(format!("{}", protocol::ADMIN_PROTOCOL_VERSION)),
    };

    Box::new(
        web::block(move || {
            let circuit = store
                .get_circuit(&circuit_id)
                .map_err(|err| CircuitFetchError::CircuitStoreError(err.to_string()))?
                .ok_or_else(|| {
                    CircuitFetchError::NotFound(format!("Unable to find circuit: {}", circuit_id))
                })?;

            Ok((circuit, protocol_version?))
        })
        .then(|res| match res {
            Ok((circuit, protocol_version)) => match protocol_version.as_str() {
                "1" => Ok(HttpResponse::Ok().json(
                    resources::v1::circuits_circuit_id::CircuitResponse::from(&circuit),
                )),
                // Handles 2 (and catch all)
                _ => Ok(HttpResponse::Ok().json(
                    resources::v2::circuits_circuit_id::CircuitResponse::from(&circuit),
                )),
            },
            Err(err) => match err {
                BlockingError::Error(err) => match err {
                    CircuitFetchError::CircuitStoreError(err) => {
                        error!("{}", err);
                        Ok(HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error()))
                    }
                    CircuitFetchError::NotFound(err) => {
                        Ok(HttpResponse::NotFound().json(ErrorResponse::not_found(&err)))
                    }
                    CircuitFetchError::BadRequest(err) => {
                        Ok(HttpResponse::BadRequest().json(ErrorResponse::bad_request(&err)))
                    }
                },

                _ => {
                    error!("{}", err);
                    Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()))
                }
            },
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use diesel::{
        r2d2::{ConnectionManager as DieselConnectionManager, Pool},
        sqlite::SqliteConnection,
    };
    use reqwest::{blocking::Client, StatusCode, Url};
    use serde_json::{to_value, Value as JsonValue};

    use crate::admin::store::diesel::DieselAdminServiceStore;
    use crate::admin::store::{
        AuthorizationType, Circuit, CircuitBuilder, CircuitNode, CircuitNodeBuilder,
        DurabilityType, PersistenceType, RouteType, ServiceBuilder,
    };
    use crate::migrations::run_sqlite_migrations;
    use crate::rest_api::{RestApiBuilder, RestApiShutdownHandle};

    #[test]
    /// Tests a GET /admin/circuit/{circuit_id} request returns the expected circuit.
    fn test_fetch_circuit_ok() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_fetch_circuit_resource(filled_splinter_state())]);

        let url = Url::parse(&format!(
            "http://{}/admin/circuits/{}",
            bind_url,
            get_circuit_1().0.circuit_id()
        ))
        .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", protocol::ADMIN_PROTOCOL_VERSION);
        let resp = req.send().expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let circuit: JsonValue = resp.json().expect("Failed to deserialize body");

        assert_eq!(
            circuit,
            to_value(resources::v2::circuits_circuit_id::CircuitResponse::from(
                &get_circuit_1().0
            ))
            .expect("failed to convert expected circuit"),
        );

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Tests a GET /admin/circuit/{circuit_id} request with protocol 1 returns the expected
    /// circuit.  This test is for backwards compatibility.
    fn test_fetch_circuit_ok_v1() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_fetch_circuit_resource(filled_splinter_state())]);

        let url = Url::parse(&format!(
            "http://{}/admin/circuits/{}",
            bind_url,
            get_circuit_1().0.circuit_id()
        ))
        .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", "1");
        let resp = req.send().expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let circuit: JsonValue = resp.json().expect("Failed to deserialize body");

        assert_eq!(
            circuit,
            to_value(resources::v1::circuits_circuit_id::CircuitResponse::from(
                &get_circuit_1().0
            ))
            .expect("failed to convert expected circuit"),
        );

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Tests a GET /admin/circuits/{circuit_id} request returns NotFound when an invalid
    /// circuit_id is passed.
    fn test_fetch_circuit_not_found() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_fetch_circuit_resource(filled_splinter_state())]);

        let url = Url::parse(&format!(
            "http://{}/admin/circuits/Circuit-not-valid",
            bind_url,
        ))
        .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", protocol::ADMIN_PROTOCOL_VERSION);
        let resp = req.send().expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    fn get_circuit_1() -> (Circuit, Vec<CircuitNode>) {
        let service = ServiceBuilder::new()
            .with_service_id("aaaa")
            .with_service_type("type_a")
            .with_node_id("node_1")
            .build()
            .expect("Unable to build service");

        let nodes = vec![
            CircuitNodeBuilder::new()
                .with_node_id("node_1")
                .with_endpoints(&["tcp://localhost:8000".to_string()])
                .build()
                .expect("Unable to build node"),
            CircuitNodeBuilder::new()
                .with_node_id("node_2")
                .with_endpoints(&["tcp://localhost:8001".to_string()])
                .build()
                .expect("Unable to build node"),
        ];

        (
            CircuitBuilder::new()
                .with_circuit_id("abcde-12345".into())
                .with_authorization_type(&AuthorizationType::Trust)
                .with_members(&["node_1".to_string(), "node_2".to_string()])
                .with_roster(&[service])
                .with_persistence(&PersistenceType::Any)
                .with_durability(&DurabilityType::NoDurability)
                .with_routes(&RouteType::Any)
                .with_circuit_management_type("circuit_1_type")
                .with_display_name("test_display")
                .build()
                .expect("Should have built a correct circuit"),
            nodes,
        )
    }

    fn get_circuit_2() -> (Circuit, Vec<CircuitNode>) {
        let service = ServiceBuilder::new()
            .with_service_id("bbbb")
            .with_service_type("other_type")
            .with_node_id("node_3")
            .build()
            .expect("unable to build service");

        let nodes = vec![
            CircuitNodeBuilder::new()
                .with_node_id("node_3")
                .with_endpoints(&["tcp://localhost:8000".to_string()])
                .build()
                .expect("Unable to build node"),
            CircuitNodeBuilder::new()
                .with_node_id("node_4")
                .with_endpoints(&["tcp://localhost:8001".to_string()])
                .build()
                .expect("Unable to build node"),
        ];

        (
            CircuitBuilder::new()
                .with_circuit_id("efghi-56789")
                .with_authorization_type(&AuthorizationType::Trust)
                .with_members(&["node_3".to_string(), "node_4".to_string()])
                .with_roster(&[service])
                .with_persistence(&PersistenceType::Any)
                .with_durability(&DurabilityType::NoDurability)
                .with_routes(&RouteType::Any)
                .with_circuit_management_type("circuit_2_type")
                .build()
                .expect("Should have built a correct circuit"),
            nodes,
        )
    }

    fn setup_admin_service_store() -> Box<dyn AdminServiceStore> {
        let connection_manager = DieselConnectionManager::<SqliteConnection>::new(":memory:");
        let pool = Pool::builder()
            .max_size(1)
            .build(connection_manager)
            .expect("Failed to build connection pool");

        run_sqlite_migrations(&*pool.get().expect("Failed to get connection for migrations"))
            .expect("Failed to run migrations");

        Box::new(DieselAdminServiceStore::new(pool))
    }

    fn filled_splinter_state() -> Box<dyn AdminServiceStore> {
        let admin_store = setup_admin_service_store();
        let (circuit, nodes) = get_circuit_1();
        admin_store
            .add_circuit(circuit, nodes)
            .expect("Unable to add circuit_1");

        let (circuit, nodes) = get_circuit_2();
        admin_store
            .add_circuit(circuit, nodes)
            .expect("Unable to add circuit_2");

        admin_store
    }

    fn run_rest_api_on_open_port(
        resources: Vec<Resource>,
    ) -> (RestApiShutdownHandle, std::thread::JoinHandle<()>, String) {
        #[cfg(not(feature = "https-bind"))]
        let bind = "127.0.0.1:0";
        #[cfg(feature = "https-bind")]
        let bind = crate::rest_api::RestApiBind::Insecure("127.0.0.1:0".into());

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
