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

//! This module provides the `GET /admin/circuits` endpoint for listing the definitions of circuits
//! in Splinter's state.

use actix_web::{error::BlockingError, web, Error, HttpRequest, HttpResponse};
use futures::{future::IntoFuture, Future};
use std::collections::HashMap;

use crate::admin::store::{AdminServiceStore, CircuitPredicate};
use crate::protocol;
use crate::rest_api::{
    paging::{get_response_paging_info, DEFAULT_LIMIT, DEFAULT_OFFSET},
    ErrorResponse, Method, ProtocolVersionRangeGuard, Resource,
};

use super::super::error::CircuitListError;
use super::super::resources;

pub fn make_list_circuits_resource(store: Box<dyn AdminServiceStore>) -> Resource {
    Resource::build("/admin/circuits")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::ADMIN_LIST_CIRCUITS_MIN,
            protocol::ADMIN_PROTOCOL_VERSION,
        ))
        .add_method(Method::Get, move |r, _| {
            list_circuits(r, web::Data::new(store.clone()))
        })
}

fn list_circuits(
    req: HttpRequest,
    store: web::Data<Box<dyn AdminServiceStore>>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    let query: web::Query<HashMap<String, String>> =
        if let Ok(q) = web::Query::from_query(req.query_string()) {
            q
        } else {
            return Box::new(
                HttpResponse::BadRequest()
                    .json(ErrorResponse::bad_request("Invalid query"))
                    .into_future(),
            );
        };

    let offset = match query.get("offset") {
        Some(value) => match value.parse::<usize>() {
            Ok(val) => val,
            Err(err) => {
                return Box::new(
                    HttpResponse::BadRequest()
                        .json(ErrorResponse::bad_request(&format!(
                            "Invalid offset value passed: {}. Error: {}",
                            value, err
                        )))
                        .into_future(),
                )
            }
        },
        None => DEFAULT_OFFSET,
    };

    let limit = match query.get("limit") {
        Some(value) => match value.parse::<usize>() {
            Ok(val) => val,
            Err(err) => {
                return Box::new(
                    HttpResponse::BadRequest()
                        .json(ErrorResponse::bad_request(&format!(
                            "Invalid limit value passed: {}. Error: {}",
                            value, err
                        )))
                        .into_future(),
                )
            }
        },
        None => DEFAULT_LIMIT,
    };

    let mut link = req.uri().path().to_string();

    let filters = match query.get("filter") {
        Some(value) => {
            link.push_str(&format!("?filter={}&", value));
            Some(value.to_string())
        }
        None => None,
    };

    let protocol_version = match req.headers().get("SplinterProtocolVersion") {
        Some(header_value) => match header_value.to_str() {
            Ok(protocol_version) => protocol_version.to_string(),
            Err(_) => {
                return Box::new(
                    HttpResponse::BadRequest()
                        .json(ErrorResponse::bad_request(
                            "Unable to get SplinterProtocolVersion",
                        ))
                        .into_future(),
                )
            }
        },
        None => format!("{}", protocol::ADMIN_PROTOCOL_VERSION),
    };

    Box::new(query_list_circuits(
        store,
        link,
        filters,
        Some(offset),
        Some(limit),
        protocol_version,
    ))
}

fn query_list_circuits(
    store: web::Data<Box<dyn AdminServiceStore>>,
    link: String,
    filters: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
    protocol_version: String,
) -> impl Future<Item = HttpResponse, Error = Error> {
    web::block(move || {
        let filters = {
            if let Some(member) = filters {
                vec![CircuitPredicate::MembersInclude(vec![member])]
            } else {
                vec![]
            }
        };

        let circuits = store
            .list_circuits(&filters)
            .map_err(|err| CircuitListError::CircuitStoreError(err.to_string()))?;

        let offset_value = offset.unwrap_or(0);
        let total = circuits.len();
        let limit_value = limit.unwrap_or_else(|| total as usize);

        let circuits = circuits
            .skip(offset_value)
            .take(limit_value)
            .collect::<Vec<_>>();

        Ok((
            circuits,
            link,
            limit,
            offset,
            total as usize,
            protocol_version,
        ))
    })
    .then(|res| match res {
        Ok((circuits, link, limit, offset, total_count, protocol_version)) => {
            match protocol_version.as_str() {
                "1" => Ok(
                    HttpResponse::Ok().json(resources::v1::circuits::ListCircuitsResponse {
                        data: circuits
                            .iter()
                            .map(resources::v1::circuits::CircuitResponse::from)
                            .collect(),
                        paging: get_response_paging_info(limit, offset, &link, total_count),
                    }),
                ),

                // Handles 2 (and catch all)
                _ => Ok(
                    HttpResponse::Ok().json(resources::v2::circuits::ListCircuitsResponse {
                        data: circuits
                            .iter()
                            .map(resources::v2::circuits::CircuitResponse::from)
                            .collect(),
                        paging: get_response_paging_info(limit, offset, &link, total_count),
                    }),
                ),
            }
        }
        Err(err) => match err {
            BlockingError::Error(err) => match err {
                CircuitListError::CircuitStoreError(err) => {
                    error!("{}", err);
                    Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()))
                }
            },
            _ => {
                error!("{}", err);
                Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()))
            }
        },
    })
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
    use crate::rest_api::{paging::Paging, RestApiBuilder, RestApiShutdownHandle};

    #[test]
    /// Tests a GET /admin/circuits request with no filters returns the expected circuits.
    fn test_list_circuits_ok() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_list_circuits_resource(filled_splinter_state())]);

        let url = Url::parse(&format!("http://{}/admin/circuits", bind_url))
            .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", protocol::ADMIN_PROTOCOL_VERSION);
        let resp = req.send().expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let circuits: JsonValue = resp.json().expect("Failed to deserialize body");

        assert_eq!(
            circuits.get("data").expect("no data field in response"),
            &to_value(vec![
                resources::v2::circuits::CircuitResponse::from(&get_circuit_2().0),
                resources::v2::circuits::CircuitResponse::from(&get_circuit_1().0)
            ])
            .expect("failed to convert expected data"),
        );
        assert_eq!(
            circuits.get("paging").expect("no paging field in response"),
            &to_value(create_test_paging_response(
                0,
                100,
                0,
                0,
                0,
                2,
                "/admin/circuits?",
            ))
            .expect("failed to convert expected paging")
        );

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Tests a GET /admin/circuits request using protocol 1, with no filters returns the expected
    /// circuits. This test is for backwards compatibility.
    fn test_list_circuits_ok_v1() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_list_circuits_resource(filled_splinter_state())]);

        let url = Url::parse(&format!("http://{}/admin/circuits", bind_url))
            .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", "1");
        let resp = req.send().expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let circuits: JsonValue = resp.json().expect("Failed to deserialize body");

        assert_eq!(
            circuits.get("data").expect("no data field in response"),
            &to_value(vec![
                resources::v1::circuits::CircuitResponse::from(&get_circuit_2().0),
                resources::v1::circuits::CircuitResponse::from(&get_circuit_1().0)
            ])
            .expect("failed to convert expected data"),
        );
        assert_eq!(
            circuits.get("paging").expect("no paging field in response"),
            &to_value(create_test_paging_response(
                0,
                100,
                0,
                0,
                0,
                2,
                "/admin/circuits?",
            ))
            .expect("failed to convert expected paging")
        );

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Tests a GET /admin/circuits request with filter returns the expected circuit.
    fn test_list_circuit_with_filters_ok() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_list_circuits_resource(filled_splinter_state())]);

        let url = Url::parse(&format!("http://{}/admin/circuits?filter=node_1", bind_url))
            .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", protocol::ADMIN_PROTOCOL_VERSION);
        let resp = req.send().expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let circuits: JsonValue = resp.json().expect("Failed to deserialize body");

        assert_eq!(
            circuits.get("data").expect("no data field in response"),
            &to_value(vec![resources::v2::circuits::CircuitResponse::from(
                &get_circuit_1().0
            )])
            .expect("failed to convert expected data"),
        );

        assert_eq!(
            circuits.get("paging").expect("no paging field in response"),
            &to_value(create_test_paging_response(
                0,
                100,
                0,
                0,
                0,
                1,
                &format!("/admin/circuits?filter=node_1&"),
            ))
            .expect("failed to convert expected paging")
        );

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Tests a GET /admin/circuits?limit=1 request returns the expected circuit.
    fn test_list_circuit_with_limit() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_list_circuits_resource(filled_splinter_state())]);

        let url = Url::parse(&format!("http://{}/admin/circuits?limit=1", bind_url))
            .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", protocol::ADMIN_PROTOCOL_VERSION);
        let resp = req.send().expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let circuits: JsonValue = resp.json().expect("Failed to deserialize body");

        assert_eq!(
            circuits.get("data").expect("no data field in response"),
            &to_value(vec![resources::v2::circuits::CircuitResponse::from(
                &get_circuit_2().0
            )])
            .expect("failed to convert expected data"),
        );

        assert_eq!(
            circuits.get("paging").expect("no paging field in response"),
            &to_value(create_test_paging_response(
                0,
                1,
                1,
                0,
                1,
                2,
                "/admin/circuits?",
            ))
            .expect("failed to convert expected paging")
        );

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Tests a GET /admin/circuits?offset=1 request returns the expected circuit.
    fn test_list_circuit_with_offset() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_list_circuits_resource(filled_splinter_state())]);

        let url = Url::parse(&format!("http://{}/admin/circuits?offset=1", bind_url))
            .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", protocol::ADMIN_PROTOCOL_VERSION);
        let resp = req.send().expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let circuits: JsonValue = resp.json().expect("Failed to deserialize body");

        assert_eq!(
            circuits.get("data").expect("no data field in response"),
            &to_value(vec![resources::v2::circuits::CircuitResponse::from(
                &get_circuit_1().0
            )])
            .expect("failed to convert expected data"),
        );

        assert_eq!(
            circuits.get("paging").expect("no paging field in response"),
            &to_value(create_test_paging_response(
                1,
                100,
                0,
                0,
                0,
                2,
                "/admin/circuits?"
            ))
            .expect("failed to convert expected paging")
        );

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    fn create_test_paging_response(
        offset: usize,
        limit: usize,
        next_offset: usize,
        previous_offset: usize,
        last_offset: usize,
        total: usize,
        link: &str,
    ) -> Paging {
        let base_link = format!("{}limit={}&", link, limit);
        let current_link = format!("{}offset={}", base_link, offset);
        let first_link = format!("{}offset=0", base_link);
        let next_link = format!("{}offset={}", base_link, next_offset);
        let previous_link = format!("{}offset={}", base_link, previous_offset);
        let last_link = format!("{}offset={}", base_link, last_offset);

        Paging {
            current: current_link,
            offset,
            limit,
            total,
            first: first_link,
            prev: previous_link,
            next: next_link,
            last: last_link,
        }
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
