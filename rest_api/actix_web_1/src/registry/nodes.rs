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

//! This module provides the following endpoints:
//!
//! * `GET /registry/nodes` for listing nodes in the registry
//! * `POST /registry/nodes` for adding a node to the registry

use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::Write as _;

use actix_web::{error::BlockingError, web, Error, HttpRequest, HttpResponse};
use futures::{future::IntoFuture, stream::Stream, Future};
use splinter::error::InvalidStateError;
use splinter::registry::{MetadataPredicate, Node, RegistryReader, RegistryWriter, RwRegistry};
use splinter_rest_api_common::{
    paging::v1::{PagingBuilder, DEFAULT_LIMIT, DEFAULT_OFFSET},
    percent_encode_filter_query::percent_encode_filter_query,
    response_models::ErrorResponse,
};

use crate::framework::{Method, ProtocolVersionRangeGuard, Resource};
use splinter_rest_api_common::SPLINTER_PROTOCOL_VERSION;

use super::error::RegistryRestApiError;
use super::resources::nodes::{ListNodesResponse, NewNode, NodeResponse};
#[cfg(feature = "authorization")]
use super::{REGISTRY_READ_PERMISSION, REGISTRY_WRITE_PERMISSION};

const REGISTRY_LIST_NODES_MIN: u32 = 1;

type Filter = HashMap<String, (String, String)>;

pub fn make_nodes_resource(registry: Box<dyn RwRegistry>) -> Resource {
    let registry1 = registry.clone();
    let resource = Resource::build("/registry/nodes").add_request_guard(
        ProtocolVersionRangeGuard::new(REGISTRY_LIST_NODES_MIN, SPLINTER_PROTOCOL_VERSION),
    );
    #[cfg(feature = "authorization")]
    {
        resource
            .add_method(Method::Get, REGISTRY_READ_PERMISSION, move |r, _| {
                list_nodes(r, web::Data::new(registry.clone_box_as_reader()))
            })
            .add_method(Method::Post, REGISTRY_WRITE_PERMISSION, move |_, p| {
                add_node(p, web::Data::new(registry1.clone()))
            })
    }
    #[cfg(not(feature = "authorization"))]
    {
        resource
            .add_method(Method::Get, move |r, _| {
                list_nodes(r, web::Data::new(registry.clone_box_as_reader()))
            })
            .add_method(Method::Post, move |_, p| {
                add_node(p, web::Data::new(registry1.clone()))
            })
    }
}

fn list_nodes(
    req: HttpRequest,
    registry: web::Data<Box<dyn RegistryReader>>,
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

    let mut link = format!("{}?", req.uri().path());

    let filters = match query.get("filter") {
        Some(value) => match serde_json::from_str(value) {
            Ok(val) => {
                if let Err(e) = write!(link, "filter={}&", percent_encode_filter_query(value)) {
                    return Box::new(
                        HttpResponse::InternalServerError()
                            .body(e.to_string())
                            .into_future(),
                    );
                }
                Some(val)
            }
            Err(err) => {
                return Box::new(
                    HttpResponse::BadRequest()
                        .json(ErrorResponse::bad_request(&format!(
                            "Invalid filter value passed: {}. Error: {}",
                            value, err
                        )))
                        .into_future(),
                )
            }
        },
        None => None,
    };

    let predicates = match to_predicates(filters) {
        Ok(predicates) => predicates,
        Err(err) => {
            return Box::new(
                HttpResponse::BadRequest()
                    .json(ErrorResponse::bad_request(&format!(
                        "Invalid predicate: {}",
                        err
                    )))
                    .into_future(),
            )
        }
    };

    Box::new(query_list_nodes(
        registry,
        link,
        predicates,
        Some(offset),
        Some(limit),
    ))
}

fn query_list_nodes(
    registry: web::Data<Box<dyn RegistryReader>>,
    link: String,
    filters: Vec<MetadataPredicate>,
    offset: Option<usize>,
    limit: Option<usize>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    web::block(move || {
        let nodes = registry
            .list_nodes(&filters)
            .map_err(RegistryRestApiError::from)?;
        let offset_value = offset.unwrap_or(0);
        let total = nodes.len();
        let limit_value = limit.unwrap_or(total as usize);

        let nodes = nodes
            .skip(offset_value)
            .take(limit_value)
            .collect::<Vec<_>>();

        Ok((nodes, link, limit, offset, total as usize))
    })
    .then(
        |res: Result<_, BlockingError<RegistryRestApiError>>| match res {
            Ok((nodes, link, limit, offset, total_count)) => {
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
                Ok(HttpResponse::Ok().json(ListNodesResponse {
                    data: nodes.iter().map(NodeResponse::from).collect(),
                    paging: paging.build(),
                }))
            }
            Err(err) => {
                error!("Unable to list nodes: {}", err);
                Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()))
            }
        },
    )
}

fn to_predicates(filters: Option<Filter>) -> Result<Vec<MetadataPredicate>, String> {
    match filters {
        Some(filters) => filters
            .into_iter()
            .map(|(key, (operator, value))| match operator.as_str() {
                "=" => Ok(MetadataPredicate::Eq(key, value)),
                ">" => Ok(MetadataPredicate::Gt(key, value)),
                "<" => Ok(MetadataPredicate::Lt(key, value)),
                ">=" => Ok(MetadataPredicate::Ge(key, value)),
                "<=" => Ok(MetadataPredicate::Le(key, value)),
                "!=" => Ok(MetadataPredicate::Ne(key, value)),
                _ => Err(format!("{} is not a valid operator", operator)),
            })
            .collect(),
        None => Ok(vec![]),
    }
}

fn add_node(
    payload: web::Payload,
    registry: web::Data<Box<dyn RwRegistry>>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    Box::new(
        payload
            .from_err::<Error>()
            .fold(web::BytesMut::new(), move |mut body, chunk| {
                body.extend_from_slice(&chunk);
                Ok::<_, Error>(body)
            })
            .into_future()
            .and_then(move |body| match serde_json::from_slice::<NewNode>(&body) {
                Ok(node) => Box::new(
                    web::block(move || {
                        let new_node = Node::try_from(node).map_err(|err| {
                            RegistryRestApiError::InvalidStateError(
                                InvalidStateError::with_message(format!(
                                    "Failed to add node, node is invalid: {}",
                                    err
                                )),
                            )
                        })?;
                        registry
                            .add_node(new_node)
                            .map_err(RegistryRestApiError::from)
                    })
                    .then(|res| {
                        Ok(match res {
                            Ok(_) => HttpResponse::Ok().finish(),
                            Err(BlockingError::Error(RegistryRestApiError::InvalidStateError(
                                err,
                            ))) => HttpResponse::BadRequest().json(ErrorResponse::bad_request(
                                &format!("Invalid node: {}", err),
                            )),
                            Err(err) => {
                                error!("Unable to add node: {}", err);
                                HttpResponse::InternalServerError()
                                    .json(ErrorResponse::internal_error())
                            }
                        })
                    }),
                )
                    as Box<dyn Future<Item = HttpResponse, Error = Error>>,
                Err(err) => Box::new(
                    HttpResponse::BadRequest()
                        .json(ErrorResponse::bad_request(&format!(
                            "Invalid node: {}",
                            err
                        )))
                        .into_future(),
                ),
            }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::{Arc, Mutex};

    use reqwest::{blocking::Client, StatusCode, Url};
    use serde_json::{to_value, Value as JsonValue};

    use splinter::error::InternalError;
    use splinter::error::InvalidStateError;
    use splinter::registry::{NodeIter, RegistryError};
    use splinter_rest_api_common::auth::{
        AuthorizationHandler, AuthorizationHandlerResult, AuthorizationHeader, Identity,
        IdentityProvider,
    };
    use splinter_rest_api_common::paging::v1::Paging;

    use crate::framework::AuthConfig;
    use crate::framework::{RestApiBuilder, RestApiShutdownHandle};

    #[test]
    /// Tests a GET /registry/nodes request with no filters returns the expected nodes.
    fn test_list_nodes_ok() {
        let (shutdown_handle, join_handle, bind_url) = run_rest_api_on_open_port(vec![
            make_nodes_resource(Box::new(MemRegistry::new(vec![get_node_1(), get_node_2()]))),
        ]);

        let url = Url::parse(&format!("http://{}/registry/nodes", bind_url))
            .expect("Failed to parse URL");
        let resp = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .header("Authorization", "custom")
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let body: JsonValue = resp.json().expect("Failed to deserialize body");

        let nodes = body
            .get("data")
            .expect("No data field in response")
            .as_array()
            .expect("data field is not an array")
            .to_vec();
        assert_eq!(2, nodes.len());
        assert!(nodes.contains(
            &to_value(NodeResponse::from(&get_node_1()))
                .expect("Failed to convert node1 to JsonValue")
        ));
        assert!(nodes.contains(
            &to_value(NodeResponse::from(&get_node_2()))
                .expect("Failed to convert node2 to JsonValue")
        ));

        assert_eq!(
            body.get("paging").expect("no paging field in response"),
            &to_value(create_test_paging_response(
                0,
                100,
                0,
                0,
                0,
                2,
                "/registry/nodes?"
            ))
            .expect("failed to convert expected paging")
        );

        shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Tests a GET /registry/nodes request with filters returns the expected node.
    fn test_list_nodes_with_filters_ok() {
        let (shutdown_handle, join_handle, bind_url) = run_rest_api_on_open_port(vec![
            make_nodes_resource(Box::new(MemRegistry::new(vec![get_node_1(), get_node_2()]))),
        ]);

        let filter = percent_encode_filter_query("{\"company\":[\"=\",\"Bitwise IO\"]}");
        let url = Url::parse(&format!(
            "http://{}/registry/nodes?filter={}",
            bind_url, filter
        ))
        .expect("Failed to parse URL");
        let resp = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .header("Authorization", "custom")
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let nodes: JsonValue = resp.json().expect("Failed to deserialize body");

        assert_eq!(
            nodes.get("data").expect("no data field in response"),
            &to_value(vec![NodeResponse::from(&get_node_1())])
                .expect("failed to convert expected data"),
        );
        assert_eq!(
            nodes.get("paging").expect("no paging field in response"),
            &to_value(create_test_paging_response(
                0,
                100,
                0,
                0,
                0,
                1,
                &format!("/registry/nodes?filter={}&", filter)
            ))
            .expect("failed to convert expected paging")
        );

        shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Tests a GET /registry/nodes request with invalid filter returns BadRequest response.
    fn test_list_node_with_filters_bad_request() {
        let (shutdown_handle, join_handle, bind_url) = run_rest_api_on_open_port(vec![
            make_nodes_resource(Box::new(MemRegistry::new(vec![get_node_1(), get_node_2()]))),
        ]);

        let filter = percent_encode_filter_query("{\"company\":[\"*\",\"Bitwise IO\"]}");
        let url = Url::parse(&format!(
            "http://{}/registry/nodes?filter={}",
            bind_url, filter
        ))
        .expect("Failed to parse URL");
        let resp = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .header("Authorization", "custom")
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Test the POST /registry/nodes route for adding a node to the registry.
    fn test_add_node() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_nodes_resource(Box::new(MemRegistry::default()))]);

        // Verify an invalid node gets a BAD_REQUEST response
        let url = Url::parse(&format!("http://{}/registry/nodes", bind_url))
            .expect("Failed to parse URL");
        let resp = Client::new()
            .post(url)
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .header("Authorization", "custom")
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // Verify a valid node gets an OK response
        let url = Url::parse(&format!("http://{}/registry/nodes", bind_url))
            .expect("Failed to parse URL");
        let resp = Client::new()
            .post(url)
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .header("Authorization", "custom")
            .json(&get_new_node_1())
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);

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
        let bind = splinter::rest_api::BindConfig::Http("127.0.0.1:0".into());

        let identity_provider = MockIdentityProvider::default().clone_box();
        let auth_config = AuthConfig::Custom {
            resources: Vec::new(),
            identity_provider,
        };
        let authorization_handlers = vec![MockAuthorizationHandler::default().clone_box()];

        let result = RestApiBuilder::new()
            .with_bind(bind)
            .add_resources(resources)
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

    fn get_node_1() -> Node {
        Node::builder("Node-123")
            .with_endpoint("12.0.0.123:8431")
            .with_display_name("Bitwise IO - Node 1")
            .with_key("0123")
            .with_metadata("company", "Bitwise IO")
            .build()
            .expect("Failed to build node1")
    }

    fn get_new_node_1() -> NewNode {
        let mut metadata = HashMap::new();
        metadata.insert("company".into(), "Bitwise IO".into());

        NewNode {
            identity: "Node-123".into(),
            endpoints: vec!["12.0.0.123:8431".into()],
            display_name: "Bitwise IO - Node 1".into(),
            keys: vec!["0123".into()],
            metadata,
        }
    }

    fn get_node_2() -> Node {
        Node::builder("Node-456")
            .with_endpoint("13.0.0.123:8434")
            .with_display_name("Cargill - Node 1")
            .with_key("abcd")
            .with_metadata("company", "Cargill")
            .build()
            .expect("Failed to build node2")
    }

    #[derive(Clone, Default)]
    struct MemRegistry {
        nodes: Arc<Mutex<HashMap<String, Node>>>,
    }

    impl MemRegistry {
        fn new(nodes: Vec<Node>) -> Self {
            let mut nodes_map = HashMap::new();
            for node in nodes {
                nodes_map.insert(node.identity().to_string().clone(), node);
            }
            Self {
                nodes: Arc::new(Mutex::new(nodes_map)),
            }
        }
    }

    impl RegistryReader for MemRegistry {
        fn list_nodes<'a, 'b: 'a>(
            &'b self,
            predicates: &'a [MetadataPredicate],
        ) -> Result<NodeIter<'a>, RegistryError> {
            let mut nodes = self
                .nodes
                .lock()
                .expect("mem registry lock was poisoned")
                .clone();
            nodes.retain(|_, node| predicates.iter().all(|predicate| predicate.apply(node)));
            Ok(Box::new(nodes.into_iter().map(|(_, node)| node)))
        }

        fn count_nodes(&self, predicates: &[MetadataPredicate]) -> Result<u32, RegistryError> {
            self.list_nodes(predicates).map(|iter| iter.count() as u32)
        }

        fn get_node(&self, identity: &str) -> Result<Option<Node>, RegistryError> {
            Ok(self
                .nodes
                .lock()
                .expect("mem registry lock was poisoned")
                .get(identity)
                .cloned())
        }
    }

    impl RegistryWriter for MemRegistry {
        fn add_node(&self, node: Node) -> Result<(), RegistryError> {
            self.nodes
                .lock()
                .expect("mem registry lock was poisoned")
                .insert(node.identity().to_string(), node);
            Ok(())
        }

        fn update_node(&self, node: Node) -> Result<(), RegistryError> {
            let mut inner = self.nodes.lock().expect("mem registry lock was poisoned");

            if let std::collections::hash_map::Entry::Occupied(mut e) =
                inner.entry(node.identity().to_string())
            {
                e.insert(node);
                Ok(())
            } else {
                Err(RegistryError::InvalidStateError(
                    InvalidStateError::with_message(format!(
                        "Node does not exist in the registry: {}",
                        node.identity()
                    )),
                ))
            }
        }

        fn delete_node(&self, identity: &str) -> Result<Option<Node>, RegistryError> {
            Ok(self
                .nodes
                .lock()
                .expect("mem registry lock was poisoned")
                .remove(identity))
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

    impl RwRegistry for MemRegistry {
        fn clone_box(&self) -> Box<dyn RwRegistry> {
            Box::new(self.clone())
        }

        fn clone_box_as_reader(&self) -> Box<dyn RegistryReader> {
            Box::new(self.clone())
        }

        fn clone_box_as_writer(&self) -> Box<dyn RegistryWriter> {
            Box::new(self.clone())
        }
    }
}
