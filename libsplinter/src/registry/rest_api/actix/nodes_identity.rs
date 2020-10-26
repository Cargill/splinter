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
//! * `GET /registry/nodes/{identity}` for fetching a node in the registry
//! * `PUT /registry/nodes/{identity}` for replacing a node in the registry
//! * `DELETE /registry/nodes/{identity}` for deleting a node from the registry

use crate::actix_web::{error::BlockingError, web, Error, HttpRequest, HttpResponse};
use crate::futures::{future::IntoFuture, stream::Stream, Future};
use crate::protocol;
use crate::registry::{
    rest_api::resources::nodes_identity::NodeResponse, InvalidNodeError, Node, RegistryError,
    RegistryReader, RegistryWriter, RwRegistry,
};
use crate::rest_api::{ErrorResponse, Method, ProtocolVersionRangeGuard, Resource};

pub fn make_nodes_identity_resource(registry: Box<dyn RwRegistry>) -> Resource {
    let registry1 = registry.clone();
    let registry2 = registry.clone();
    Resource::build("/registry/nodes/{identity}")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::REGISTRY_FETCH_NODE_MIN,
            protocol::REGISTRY_PROTOCOL_VERSION,
        ))
        .add_method(Method::Get, move |r, _| {
            fetch_node(r, web::Data::new(registry.clone_box_as_reader()))
        })
        .add_method(Method::Put, move |r, p| {
            put_node(r, p, web::Data::new(registry1.clone_box_as_writer()))
        })
        .add_method(Method::Delete, move |r, _| {
            delete_node(r, web::Data::new(registry2.clone_box_as_writer()))
        })
}

fn fetch_node(
    request: HttpRequest,
    registry: web::Data<Box<dyn RegistryReader>>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    let identity = request
        .match_info()
        .get("identity")
        .unwrap_or("")
        .to_string();
    Box::new(
        web::block(move || registry.fetch_node(&identity)).then(|res| {
            Ok(match res {
                Ok(Some(node)) => HttpResponse::Ok().json(NodeResponse::from(&node)),
                Ok(None) => {
                    HttpResponse::NotFound().json(ErrorResponse::not_found("Node not found"))
                }
                Err(err) => {
                    error!("Unable to fetch node: {}", err);
                    HttpResponse::InternalServerError().json(ErrorResponse::internal_error())
                }
            })
        }),
    )
}

fn put_node(
    request: HttpRequest,
    payload: web::Payload,
    registry: web::Data<Box<dyn RegistryWriter>>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    let path_identity = request
        .match_info()
        .get("identity")
        .unwrap_or("")
        .to_string();
    Box::new(
        payload
            .from_err::<Error>()
            .fold(web::BytesMut::new(), move |mut body, chunk| {
                body.extend_from_slice(&chunk);
                Ok::<_, Error>(body)
            })
            .into_future()
            .and_then(move |body| match serde_json::from_slice::<Node>(&body) {
                Ok(node) => Box::new(
                    web::block(move || {
                        if node.identity != path_identity {
                            Err(RegistryError::InvalidNode(
                                InvalidNodeError::InvalidIdentity(
                                    node.identity,
                                    "Node identity cannot be changed".into(),
                                ),
                            ))
                        } else {
                            registry.insert_node(node)
                        }
                    })
                    .then(|res| {
                        Ok(match res {
                            Ok(_) => HttpResponse::Ok().finish(),
                            Err(BlockingError::Error(RegistryError::InvalidNode(err))) => {
                                HttpResponse::BadRequest().json(ErrorResponse::bad_request(
                                    &format!("Invalid node: {}", err),
                                ))
                            }
                            Err(err) => {
                                error!("Unable to put node: {}", err);
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

fn delete_node(
    request: HttpRequest,
    registry: web::Data<Box<dyn RegistryWriter>>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    let identity = request
        .match_info()
        .get("identity")
        .unwrap_or("")
        .to_string();
    Box::new(
        web::block(move || registry.delete_node(&identity)).then(|res| {
            Ok(match res {
                Ok(Some(_)) => HttpResponse::Ok().finish(),
                Ok(None) => {
                    HttpResponse::NotFound().json(ErrorResponse::not_found("Node not found"))
                }
                Err(err) => {
                    error!("Unable to delete node: {}", err);
                    HttpResponse::InternalServerError().json(ErrorResponse::internal_error())
                }
            })
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use reqwest::{blocking::Client, StatusCode, Url};

    use crate::registry::{MetadataPredicate, NodeIter};
    use crate::rest_api::{RestApiBuilder, RestApiServerError, RestApiShutdownHandle};

    #[test]
    /// Tests a GET /registry/nodes/{identity} request returns the expected node.
    fn test_fetch_node_ok() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_nodes_identity_resource(Box::new(
                MemRegistry::new(vec![get_node_1(), get_node_2()]),
            ))]);

        let url = Url::parse(&format!(
            "http://{}/registry/nodes/{}",
            bind_url,
            get_node_1().identity
        ))
        .expect("Failed to parse URL");
        let resp = Client::new()
            .get(url)
            .header(
                "SplinterProtocolVersion",
                protocol::REGISTRY_PROTOCOL_VERSION,
            )
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let node: Node = resp.json().expect("Failed to deserialize body");
        assert_eq!(node, get_node_1());

        shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Tests a GET /registry/nodes/{identity} request returns NotFound when an invalid identity is
    /// passed.
    fn test_fetch_node_not_found() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_nodes_identity_resource(Box::new(
                MemRegistry::new(vec![get_node_1(), get_node_2()]),
            ))]);

        let url = Url::parse(&format!(
            "http://{}/registry/nodes/Node-not-valid",
            bind_url
        ))
        .expect("Failed to parse URL");
        let resp = Client::new()
            .get(url)
            .header(
                "SplinterProtocolVersion",
                protocol::REGISTRY_PROTOCOL_VERSION,
            )
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Test the PUT /registry/nodes/{identity} route for adding or updating a node in the registry.
    fn test_put_node() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_nodes_identity_resource(Box::new(
                MemRegistry::new(vec![get_node_1()]),
            ))]);

        // Verify no body (i.e. no updated Node) gets a BAD_REQUEST response
        let url = Url::parse(&format!(
            "http://{}/registry/nodes/{}",
            bind_url,
            get_node_1().identity
        ))
        .expect("Failed to parse URL");
        let resp = Client::new()
            .put(url)
            .header(
                "SplinterProtocolVersion",
                protocol::REGISTRY_PROTOCOL_VERSION,
            )
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // Verify that updating an existing node gets an OK response and the fetched node has
        // the updated values
        let mut node = get_node_1();
        node.endpoints = vec!["12.0.0.123:8432".to_string()];
        node.metadata
            .insert("location".to_string(), "Minneapolis".to_string());

        let url = Url::parse(&format!(
            "http://{}/registry/nodes/{}",
            bind_url, &node.identity
        ))
        .expect("Failed to parse URL");
        let resp = Client::new()
            .put(url.clone())
            .header(
                "SplinterProtocolVersion",
                protocol::REGISTRY_PROTOCOL_VERSION,
            )
            .json(&node)
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);

        let resp = Client::new()
            .get(url)
            .header(
                "SplinterProtocolVersion",
                protocol::REGISTRY_PROTOCOL_VERSION,
            )
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let updated_node: Node = resp.json().expect("Failed to deserialize body");
        assert_eq!(updated_node, node);

        // Verify that attempting to change the node identity gets a FORBIDDEN response
        let old_identity = node.identity.clone();
        node.identity = "Node-789".into();

        let url = Url::parse(&format!(
            "http://{}/registry/nodes/{}",
            bind_url, old_identity
        ))
        .expect("Failed to parse URL");
        let resp = Client::new()
            .put(url)
            .header(
                "SplinterProtocolVersion",
                protocol::REGISTRY_PROTOCOL_VERSION,
            )
            .json(&node)
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Test the DELETE /registry/nodes/{identity} route for deleting a node from the registry.
    fn test_delete_node() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_nodes_identity_resource(Box::new(
                MemRegistry::new(vec![get_node_1()]),
            ))]);

        // Verify that an existing node gets an OK response
        let url = Url::parse(&format!(
            "http://{}/registry/nodes/{}",
            bind_url,
            get_node_1().identity
        ))
        .expect("Failed to parse URL");
        let resp = Client::new()
            .delete(url.clone())
            .header(
                "SplinterProtocolVersion",
                protocol::REGISTRY_PROTOCOL_VERSION,
            )
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);

        // Verify that a non-existent node gets a NOT_FOUND response
        let resp = Client::new()
            .delete(url)
            .header(
                "SplinterProtocolVersion",
                protocol::REGISTRY_PROTOCOL_VERSION,
            )
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    fn run_rest_api_on_open_port(
        resources: Vec<Resource>,
    ) -> (RestApiShutdownHandle, std::thread::JoinHandle<()>, String) {
        (10000..20000)
            .find_map(|port| {
                let bind_url = format!("127.0.0.1:{}", port);
                let result = RestApiBuilder::new()
                    .with_bind(&bind_url)
                    .add_resources(resources.clone())
                    .build_insecure()
                    .expect("Failed to build REST API")
                    .run_insecure();
                match result {
                    Ok((shutdown_handle, join_handle)) => {
                        Some((shutdown_handle, join_handle, bind_url))
                    }
                    Err(RestApiServerError::BindError(_)) => None,
                    Err(err) => panic!("Failed to run REST API: {}", err),
                }
            })
            .expect("No port available")
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
                nodes_map.insert(node.identity.clone(), node);
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

        fn fetch_node(&self, identity: &str) -> Result<Option<Node>, RegistryError> {
            Ok(self
                .nodes
                .lock()
                .expect("mem registry lock was poisoned")
                .get(identity)
                .cloned())
        }
    }

    impl RegistryWriter for MemRegistry {
        fn insert_node(&self, node: Node) -> Result<(), RegistryError> {
            self.nodes
                .lock()
                .expect("mem registry lock was poisoned")
                .insert(node.identity.clone(), node);
            Ok(())
        }

        fn delete_node(&self, identity: &str) -> Result<Option<Node>, RegistryError> {
            Ok(self
                .nodes
                .lock()
                .expect("mem registry lock was poisoned")
                .remove(identity))
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
