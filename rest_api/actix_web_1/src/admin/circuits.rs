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

//! This module provides the `GET /admin/circuits` endpoint for listing the definitions of circuits
//! in Splinter's state.

use std::fmt::Write as _;

use actix_web::{error::BlockingError, web, Error, HttpRequest, HttpResponse};
use futures::{future::IntoFuture, Future};
use std::collections::HashMap;

use crate::framework::{Method, ProtocolVersionRangeGuard, Resource};
use splinter::admin::store::{AdminServiceStore, CircuitPredicate, CircuitStatus};
use splinter_rest_api_common::{
    paging::v1::{PagingBuilder, DEFAULT_LIMIT, DEFAULT_OFFSET},
    response_models::ErrorResponse,
    SPLINTER_PROTOCOL_VERSION,
};

use super::error::CircuitListError;
use super::resources;
#[cfg(feature = "authorization")]
use super::CIRCUIT_READ_PERMISSION;

const ADMIN_LIST_CIRCUITS_MIN: u32 = 1;

pub fn make_list_circuits_resource(store: Box<dyn AdminServiceStore>) -> Resource {
    let resource = Resource::build("/admin/circuits").add_request_guard(
        ProtocolVersionRangeGuard::new(ADMIN_LIST_CIRCUITS_MIN, SPLINTER_PROTOCOL_VERSION),
    );
    #[cfg(feature = "authorization")]
    {
        resource.add_method(Method::Get, CIRCUIT_READ_PERMISSION, move |r, _| {
            list_circuits(r, web::Data::new(store.clone()))
        })
    }
    #[cfg(not(feature = "authorization"))]
    {
        resource.add_method(Method::Get, move |r, _| {
            list_circuits(r, web::Data::new(store.clone()))
        })
    }
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

    let mut new_queries = vec![];
    let member_filter = match query.get("filter") {
        Some(value) => {
            new_queries.push(format!("filter={}", value));
            Some(value.to_string())
        }
        None => None,
    };

    let status_filter = match query.get("status") {
        Some(value) => {
            new_queries.push(format!("status={}", value));
            Some(value.to_string())
        }
        None => None,
    };
    let mut link = req.uri().path().to_string();
    if !new_queries.is_empty() {
        if let Err(e) = write!(link, "?{}&", new_queries.join("&")) {
            return Box::new(
                HttpResponse::InternalServerError()
                    .body(e.to_string())
                    .into_future(),
            );
        }
    }

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
        None => format!("{}", SPLINTER_PROTOCOL_VERSION),
    };

    Box::new(query_list_circuits(
        store,
        link,
        member_filter,
        status_filter,
        Some(offset),
        Some(limit),
        protocol_version,
    ))
}

fn query_list_circuits(
    store: web::Data<Box<dyn AdminServiceStore>>,
    link: String,
    member_filter: Option<String>,
    status_filter: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
    protocol_version: String,
) -> impl Future<Item = HttpResponse, Error = Error> {
    web::block(move || {
        let mut filters = {
            if let Some(member) = member_filter {
                vec![CircuitPredicate::MembersInclude(vec![member])]
            } else {
                vec![]
            }
        };
        if let Some(status) = status_filter {
            filters.push(CircuitPredicate::CircuitStatus(
                CircuitStatus::try_from(status)
                    .map_err(|e| CircuitListError::CircuitStatusError(e.to_string()))?,
            ));
        }

        let circuits = store
            .list_circuits(&filters)
            .map_err(|err| CircuitListError::CircuitStoreError(err.to_string()))?;

        let offset_value = offset.unwrap_or(0);
        let total = circuits.len();
        let limit_value = limit.unwrap_or(total as usize);

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
                "1" => {
                    let paging = PagingBuilder::new(link, total_count);
                    let paging = if let Some(limit) = limit {
                        paging.with_limit(limit)
                    } else {
                        paging
                    };
                    let paging = if let Some(offset) = offset {
                        paging.with_offset(offset)
                    } else {
                        paging
                    };
                    Ok(
                        HttpResponse::Ok().json(resources::v1::circuits::ListCircuitsResponse {
                            data: circuits
                                .iter()
                                .map(resources::v1::circuits::CircuitResponse::from)
                                .collect(),
                            paging: paging.build(),
                        }),
                    )
                }

                // Handles 2
                "2" => {
                    let paging = PagingBuilder::new(link, total_count);
                    let paging = if let Some(limit) = limit {
                        paging.with_limit(limit)
                    } else {
                        paging
                    };
                    let paging = if let Some(offset) = offset {
                        paging.with_offset(offset)
                    } else {
                        paging
                    };
                    Ok(
                        HttpResponse::Ok().json(resources::v2::circuits::ListCircuitsResponse {
                            data: circuits
                                .iter()
                                .map(resources::v2::circuits::CircuitResponse::from)
                                .collect(),
                            paging: paging.build(),
                        }),
                    )
                }
                _ => Ok(
                    HttpResponse::BadRequest().json(ErrorResponse::bad_request(&format!(
                        "Unsupported SplinterProtocolVersion: {}",
                        protocol_version
                    ))),
                ),
            }
        }
        Err(err) => match err {
            BlockingError::Error(err) => match err {
                CircuitListError::CircuitStoreError(err) => {
                    error!("{}", err);
                    Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()))
                }
                CircuitListError::CircuitStatusError(msg) => {
                    error!("{msg}");
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

    use crate::framework::AuthConfig;
    use crate::framework::{RestApiBuilder, RestApiShutdownHandle};
    use splinter::admin::store::diesel::DieselAdminServiceStore;
    use splinter::admin::store::{
        AuthorizationType, Circuit, CircuitBuilder, CircuitNode, CircuitNodeBuilder,
        DurabilityType, PersistenceType, RouteType, ServiceBuilder,
    };
    use splinter::error::InternalError;
    use splinter::migrations::run_sqlite_migrations;
    use splinter_rest_api_common::auth::AuthorizationHeader;
    use splinter_rest_api_common::auth::{AuthorizationHandler, AuthorizationHandlerResult};
    use splinter_rest_api_common::auth::{Identity, IdentityProvider};
    use splinter_rest_api_common::paging::v1::Paging;

    #[test]
    /// Tests a GET /admin/circuits request with no filters returns the expected circuits.
    fn test_list_circuits_ok() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_list_circuits_resource(filled_splinter_state())]);

        let url = Url::parse(&format!("http://{}/admin/circuits", bind_url))
            .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("Authorization", "custom")
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION);
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
            .header("Authorization", "custom")
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
            .header("Authorization", "custom")
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION);
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
                &"/admin/circuits?filter=node_1&".to_string(),
            ))
            .expect("failed to convert expected paging")
        );

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Tests a GET /admin/circuits request with the `status` filter returns the expected circuit.
    fn test_list_circuit_with_status_ok() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_list_circuits_resource(filled_splinter_state())]);

        let url = Url::parse(&format!(
            "http://{}/admin/circuits?status=disbanded",
            bind_url
        ))
        .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("Authorization", "custom")
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION);
        let resp = req.send().expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let circuits: JsonValue = resp.json().expect("Failed to deserialize body");

        assert_eq!(
            circuits.get("data").expect("no data field in response"),
            &to_value(vec![resources::v2::circuits::CircuitResponse::from(
                &get_circuit_3().0
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
                &"/admin/circuits?status=disbanded&".to_string(),
            ))
            .expect("failed to convert expected paging")
        );

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Tests a GET /admin/circuits request with the `status` filter returns the expected circuit.
    fn test_list_circuit_with_filter_and_status_ok() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_list_circuits_resource(filled_splinter_state())]);

        let url = Url::parse(&format!(
            "http://{}/admin/circuits?filter=node_5&\
                status=disbanded",
            bind_url
        ))
        .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("Authorization", "custom")
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION);
        let resp = req.send().expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let circuits: JsonValue = resp.json().expect("Failed to deserialize body");

        assert_eq!(
            circuits.get("data").expect("no data field in response"),
            &to_value(vec![resources::v2::circuits::CircuitResponse::from(
                &get_circuit_3().0
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
                &"/admin/circuits?filter=node_5&status=disbanded&".to_string(),
            ))
            .expect("failed to convert expected paging")
        );

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Tests a GET /admin/circuits request with a member filter and `status` filter returns no
    /// circuit if both filters are not matched.
    fn test_list_circuit_with_filter_and_status_none() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_list_circuits_resource(filled_splinter_state())]);

        let url = Url::parse(&format!(
            "http://{}/admin/circuits?filter=node_5&\
                status=active",
            bind_url
        ))
        .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("Authorization", "custom")
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION);
        let resp = req.send().expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let circuits: JsonValue = resp.json().expect("Failed to deserialize body");
        let empty_value: Vec<String> = vec![];
        assert_eq!(
            circuits.get("data").expect("no data field in response"),
            &to_value(empty_value).expect("failed to convert expected data"),
        );

        assert_eq!(
            circuits.get("paging").expect("no paging field in response"),
            &to_value(create_test_paging_response(
                0,
                100,
                0,
                0,
                0,
                0,
                &"/admin/circuits?filter=node_5&status=active&".to_string(),
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
            .header("Authorization", "custom")
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION);
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
            .header("Authorization", "custom")
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION);
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
        _next_offset: usize,
        _previous_offset: usize,
        _last_offset: usize,
        total: usize,
        link: &str,
    ) -> Paging {
        Paging::builder(link.to_string(), total)
            .with_limit(limit)
            .with_offset(offset)
            .build()
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
                .with_circuit_id("abcde-12345")
                .with_authorization_type(&AuthorizationType::Trust)
                .with_members(&nodes)
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
                .with_members(&nodes)
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

    fn get_circuit_3() -> (Circuit, Vec<CircuitNode>) {
        let service = ServiceBuilder::new()
            .with_service_id("cccc")
            .with_service_type("other_type")
            .with_node_id("node_5")
            .build()
            .expect("unable to build service");

        let nodes = vec![
            CircuitNodeBuilder::new()
                .with_node_id("node_5")
                .with_endpoints(&["tcp://localhost:8000".to_string()])
                .build()
                .expect("Unable to build node"),
            CircuitNodeBuilder::new()
                .with_node_id("node_6")
                .with_endpoints(&["tcp://localhost:8001".to_string()])
                .build()
                .expect("Unable to build node"),
        ];

        (
            CircuitBuilder::new()
                .with_circuit_id("efghi-12345")
                .with_authorization_type(&AuthorizationType::Trust)
                .with_members(&nodes)
                .with_roster(&[service])
                .with_persistence(&PersistenceType::Any)
                .with_durability(&DurabilityType::NoDurability)
                .with_routes(&RouteType::Any)
                .with_circuit_management_type("circuit_3_type")
                .with_circuit_status(&CircuitStatus::Disbanded)
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

        let (circuit, nodes) = get_circuit_3();
        admin_store
            .add_circuit(circuit, nodes)
            .expect("Unable to add circuit_3");

        admin_store
    }

    fn run_rest_api_on_open_port(
        resources: Vec<Resource>,
    ) -> (RestApiShutdownHandle, std::thread::JoinHandle<()>, String) {
        #[cfg(not(feature = "https-bind"))]
        let bind = "127.0.0.1:0";
        #[cfg(feature = "https-bind")]
        let bind = splinter::rest_api::BindConfig::Http("127.0.0.1:0".into());

        let identity_provider = MockIdentityProvider::default().clone_box();
        let auth_config = AuthConfig::Custom {
            resources: Vec::new(),
            identity_provider,
        };

        let authorization_handlers = vec![MockAuthorizationHandler::default().clone_box()];

        let result = RestApiBuilder::new()
            .with_bind(bind)
            .add_resources(resources.clone())
            .push_auth_config(auth_config)
            .with_authorization_handlers(authorization_handlers)
            .build()
            .expect("Failed to build REST API")
            .run();
        match result {
            Ok((shutdown_handle, join_handle)) => {
                let port = shutdown_handle.port_numbers()[0];
                (shutdown_handle, join_handle, format!("127.0.0.1:{}", port))
            }
            Err(err) => panic!("Failed to run REST API: {}", err),
        }
    }

    #[derive(Clone, Default)]
    struct MockIdentityProvider {}

    impl IdentityProvider for MockIdentityProvider {
        fn get_identity(
            &self,
            _authorization: &AuthorizationHeader,
        ) -> Result<Option<Identity>, InternalError> {
            Ok(Some(Identity::Custom("custom".to_string())))
        }
        fn clone_box(&self) -> Box<dyn IdentityProvider> {
            Box::new(self.clone())
        }
    }

    #[derive(Clone, Default)]
    struct MockAuthorizationHandler {}

    impl AuthorizationHandler for MockAuthorizationHandler {
        fn has_permission(
            &self,
            _identity: &Identity,
            _permission_id: &str,
        ) -> Result<AuthorizationHandlerResult, InternalError> {
            Ok(AuthorizationHandlerResult::Allow)
        }
        fn clone_box(&self) -> Box<dyn AuthorizationHandler> {
            Box::new(self.clone())
        }
    }
}
