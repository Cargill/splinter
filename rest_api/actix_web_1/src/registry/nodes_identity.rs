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
//! * `GET /registry/nodes/{identity}` for fetching a node in the registry
//! * `PUT /registry/nodes/{identity}` for replacing a node in the registry
//! * `DELETE /registry/nodes/{identity}` for deleting a node from the registry

use std::convert::TryFrom;

use actix_web::{error::BlockingError, web, Error, HttpRequest, HttpResponse};
use futures::{future::IntoFuture, stream::Stream, Future};

use crate::framework::{Method, ProtocolVersionRangeGuard, Resource};
use splinter::error::InvalidStateError;
use splinter::registry::{Node, RegistryReader, RegistryWriter, RwRegistry};
use splinter_rest_api_common::response_models::ErrorResponse;
use splinter_rest_api_common::SPLINTER_PROTOCOL_VERSION;

use super::error::RegistryRestApiError;
use super::resources::nodes_identity::{NewNode, NodeResponse};
#[cfg(feature = "authorization")]
use super::{REGISTRY_READ_PERMISSION, REGISTRY_WRITE_PERMISSION};

const REGISTRY_FETCH_NODE_MIN: u32 = 1;

pub fn make_nodes_identity_resource(registry: Box<dyn RwRegistry>) -> Resource {
    let registry1 = registry.clone();
    let registry2 = registry.clone();
    let resource = Resource::build("/registry/nodes/{identity}").add_request_guard(
        ProtocolVersionRangeGuard::new(REGISTRY_FETCH_NODE_MIN, SPLINTER_PROTOCOL_VERSION),
    );
    #[cfg(feature = "authorization")]
    {
        resource
            .add_method(Method::Get, REGISTRY_READ_PERMISSION, move |r, _| {
                fetch_node(r, web::Data::new(registry.clone_box_as_reader()))
            })
            .add_method(Method::Put, REGISTRY_WRITE_PERMISSION, move |r, p| {
                put_node(r, p, web::Data::new(registry1.clone_box_as_writer()))
            })
            .add_method(Method::Delete, REGISTRY_WRITE_PERMISSION, move |r, _| {
                delete_node(r, web::Data::new(registry2.clone_box_as_writer()))
            })
    }
    #[cfg(not(feature = "authorization"))]
    {
        resource
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
        web::block(move || {
            registry
                .get_node(&identity)
                .map_err(RegistryRestApiError::from)
        })
        .then(|res| {
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
            .and_then(move |body| match serde_json::from_slice::<NewNode>(&body) {
                Ok(node) => Box::new(
                    web::block(move || {
                        let update_node = Node::try_from(node).map_err(|err| {
                            RegistryRestApiError::InvalidStateError(
                                InvalidStateError::with_message(format!(
                                    "Failed to update node, node is invalid: {}",
                                    err
                                )),
                            )
                        })?;

                        if update_node.identity() != path_identity {
                            Err(RegistryRestApiError::InvalidStateError(
                                InvalidStateError::with_message(format!(
                                    "Node identity cannot be changed: {}",
                                    update_node.identity()
                                )),
                            ))
                        } else {
                            registry
                                .update_node(update_node)
                                .map_err(RegistryRestApiError::from)
                        }
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
        web::block(move || {
            registry
                .delete_node(&identity)
                .map_err(RegistryRestApiError::from)
        })
        .then(|res| {
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

    use splinter::error::InternalError;
    use splinter::error::InvalidStateError;
    use splinter::registry::{MetadataPredicate, NodeIter, RegistryError};
    use splinter_rest_api_common::auth::{AuthorizationHandler, AuthorizationHandlerResult};
    use splinter_rest_api_common::auth::{AuthorizationHeader, Identity, IdentityProvider};

    use crate::framework::{AuthConfig, RestApiBuilder, RestApiShutdownHandle};

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
            get_node_1().identity()
        ))
        .expect("Failed to parse URL");
        let resp = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .header("Authorization", "custom")
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let node: TestNode = resp.json().expect("Failed to deserialize body");
        assert_eq!(
            Node::try_from(node).expect("Unable to build node"),
            get_node_1()
        );

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
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .header("Authorization", "custom")
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
            get_node_1().identity()
        ))
        .expect("Failed to parse URL");
        let resp = Client::new()
            .put(url)
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .header("Authorization", "custom")
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // Verify that updating an existing node gets an OK response and the fetched node has
        // the updated values
        let mut node = get_new_node_1();
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
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .header("Authorization", "custom")
            .json(&node)
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);

        let resp = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .header("Authorization", "custom")
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let updated_node: NewNode = resp.json().expect("Failed to deserialize body");
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
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .header("Authorization", "custom")
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
            get_node_1().identity()
        ))
        .expect("Failed to parse URL");
        let resp = Client::new()
            .delete(url.clone())
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .header("Authorization", "custom")
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);

        // Verify that a non-existent node gets a NOT_FOUND response
        let resp = Client::new()
            .delete(url)
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .header("Authorization", "custom")
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

    /// Test representation of a node in a registry.
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    struct TestNode {
        identity: String,
        endpoints: Vec<String>,
        display_name: String,
        keys: Vec<String>,
        metadata: HashMap<String, String>,
    }

    impl TryFrom<TestNode> for Node {
        type Error = String;

        fn try_from(node: TestNode) -> Result<Self, Self::Error> {
            let mut builder = Node::builder(node.identity)
                .with_endpoints(node.endpoints)
                .with_display_name(node.display_name)
                .with_keys(node.keys);

            for (k, v) in node.metadata {
                builder = builder.with_metadata(k, v);
            }

            builder.build().map_err(|err| err.to_string())
        }
    }

    #[derive(Clone, Default)]
    struct MemRegistry {
        nodes: Arc<Mutex<HashMap<String, Node>>>,
    }

    impl MemRegistry {
        fn new(nodes: Vec<Node>) -> Self {
            let mut nodes_map = HashMap::new();
            for node in nodes {
                nodes_map.insert(node.identity().to_string(), node);
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

            if inner.contains_key(node.identity()) {
                inner.insert(node.identity().to_string(), node);
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
