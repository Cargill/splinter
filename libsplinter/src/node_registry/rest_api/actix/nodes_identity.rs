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

use crate::actix_web::{error::BlockingError, web, Error, HttpRequest, HttpResponse};
use crate::futures::{future::IntoFuture, stream::Stream, Future};
use crate::node_registry::{
    error::{InvalidNodeError, NodeRegistryError},
    Node, NodeRegistryReader, NodeRegistryWriter,
};
use crate::protocol;
use crate::rest_api::{Method, ProtocolVersionRangeGuard, Resource};

pub fn make_nodes_identity_resource<N>(registry: N) -> Resource
where
    N: NodeRegistryReader + NodeRegistryWriter + Clone + 'static,
{
    let registry1 = registry.clone();
    let registry2 = registry.clone();
    Resource::build("/admin/nodes/{identity}")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::ADMIN_FETCH_NODE_MIN,
            protocol::ADMIN_PROTOCOL_VERSION,
        ))
        .add_method(Method::Get, move |r, _| {
            fetch_node(r, web::Data::new(registry.clone()))
        })
        .add_method(Method::Put, move |r, p| {
            put_node(r, p, web::Data::new(registry1.clone()))
        })
        .add_method(Method::Delete, move |r, _| {
            delete_node(r, web::Data::new(registry2.clone()))
        })
}

fn fetch_node<NR>(
    request: HttpRequest,
    registry: web::Data<NR>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>>
where
    NR: NodeRegistryReader + 'static,
{
    let identity = request
        .match_info()
        .get("identity")
        .unwrap_or("")
        .to_string();
    Box::new(
        web::block(move || registry.fetch_node(&identity)).then(|res| match res {
            Ok(Some(node)) => Ok(HttpResponse::Ok().json(node)),
            Ok(None) => Ok(HttpResponse::NotFound().json("node not found")),
            Err(err) => Ok(HttpResponse::InternalServerError().json(format!("{}", err))),
        }),
    )
}

fn put_node<NW>(
    request: HttpRequest,
    payload: web::Payload,
    registry: web::Data<NW>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>>
where
    NW: NodeRegistryWriter + 'static,
{
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
                            Err(NodeRegistryError::InvalidNode(
                                InvalidNodeError::InvalidIdentity(
                                    node.identity,
                                    "node identity cannot be changed".into(),
                                ),
                            ))
                        } else {
                            registry.insert_node(node)
                        }
                    })
                    .then(|res| {
                        Ok(match res {
                            Ok(_) => HttpResponse::Ok().finish(),
                            Err(err) => match err {
                                BlockingError::Error(err) => match err {
                                    NodeRegistryError::InvalidNode(err) => {
                                        HttpResponse::Forbidden()
                                            .json(format!("node is invalid: {}", err))
                                    }
                                    _ => {
                                        HttpResponse::InternalServerError().json(format!("{}", err))
                                    }
                                },
                                _ => HttpResponse::InternalServerError().json(format!("{}", err)),
                            },
                        })
                    }),
                )
                    as Box<dyn Future<Item = HttpResponse, Error = Error>>,
                Err(err) => Box::new(
                    HttpResponse::BadRequest()
                        .json(format!("invalid node: {}", err))
                        .into_future(),
                ),
            }),
    )
}

fn delete_node<NW>(
    request: HttpRequest,
    registry: web::Data<NW>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>>
where
    NW: NodeRegistryWriter + 'static,
{
    let identity = request
        .match_info()
        .get("identity")
        .unwrap_or("")
        .to_string();
    Box::new(
        web::block(move || registry.delete_node(&identity)).then(|res| match res {
            Ok(Some(_)) => Ok(HttpResponse::Ok().finish()),
            Ok(None) => Ok(HttpResponse::NotFound().json("node not found")),
            Err(err) => Ok(HttpResponse::InternalServerError().json(format!("{}", err))),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::env;
    use std::fs::{remove_file, File};
    use std::panic;
    use std::thread;

    use crate::actix_web::{
        http::{header, StatusCode},
        test, web, App,
    };
    use crate::node_registry::{LocalYamlNodeRegistry, NodeBuilder};

    fn new_yaml_node_registry(file_path: &str) -> LocalYamlNodeRegistry {
        LocalYamlNodeRegistry::new(file_path).expect("Error creating LocalYamlNodeRegistry")
    }

    #[test]
    /// Tests a GET /admin/nodes/{identity} request returns the expected node.
    fn test_fetch_node_ok() {
        run_test(|test_yaml_file_path| {
            write_to_file(&test_yaml_file_path, &[get_node_1(), get_node_2()]);

            let node_registry = new_yaml_node_registry(test_yaml_file_path);

            let mut app = test::init_service(
                App::new().data(node_registry.clone()).service(
                    web::resource("/admin/nodes/{identity}")
                        .route(web::get().to_async(fetch_node::<LocalYamlNodeRegistry>)),
                ),
            );

            let req = test::TestRequest::get()
                .uri(&format!("/admin/nodes/{}", get_node_1().identity))
                .to_request();

            let resp = test::call_service(&mut app, req);

            assert_eq!(resp.status(), StatusCode::OK);
            let node: Node = serde_yaml::from_slice(&test::read_body(resp)).unwrap();
            assert_eq!(node, get_node_1())
        })
    }

    #[test]
    /// Tests a GET /admin/nodes/{identity} request returns NotFound when an invalid identity is passed
    fn test_fetch_node_not_found() {
        run_test(|test_yaml_file_path| {
            write_to_file(&test_yaml_file_path, &[get_node_1(), get_node_2()]);

            let node_registry = new_yaml_node_registry(test_yaml_file_path);

            let mut app = test::init_service(
                App::new().data(node_registry.clone()).service(
                    web::resource("/admin/nodes/{identity}")
                        .route(web::get().to_async(fetch_node::<LocalYamlNodeRegistry>)),
                ),
            );

            let req = test::TestRequest::get()
                .uri("/admin/nodes/Node-not-valid")
                .to_request();

            let resp = test::call_service(&mut app, req);

            assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        })
    }

    #[test]
    /// Test the PUT /admin/nodes/{identity} route for adding or updating a node in the registry.
    fn test_put_node() {
        run_test(|test_yaml_file_path| {
            let mut node = get_node_1();
            write_to_file(&test_yaml_file_path, &[node.clone()]);

            let node_registry = new_yaml_node_registry(test_yaml_file_path);

            let mut app = test::init_service(
                App::new().data(node_registry.clone()).service(
                    web::resource("/admin/nodes/{identity}")
                        .route(web::patch().to_async(put_node::<LocalYamlNodeRegistry>))
                        .route(web::get().to_async(fetch_node::<LocalYamlNodeRegistry>)),
                ),
            );

            // Verify no body (e.g. no updated Node) gets a BAD_REQUEST response
            let req = test::TestRequest::patch()
                .uri(&format!("/admin/nodes/{}", &node.identity))
                .to_request();

            let resp = test::call_service(&mut app, req);

            assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

            // Verify that updating an existing node gets an OK response and the fetched node has
            // the updated values
            node.endpoints = vec!["12.0.0.123:8432".to_string()];
            node.metadata
                .insert("location".to_string(), "Minneapolis".to_string());

            let req = test::TestRequest::patch()
                .uri(&format!("/admin/nodes/{}", &node.identity))
                .header(header::CONTENT_TYPE, "application/json")
                .set_json(&node)
                .to_request();

            let resp = test::call_service(&mut app, req);

            assert_eq!(resp.status(), StatusCode::OK);

            let req = test::TestRequest::get()
                .uri(&format!("/admin/nodes/{}", &node.identity))
                .to_request();

            let resp = test::call_service(&mut app, req);

            assert_eq!(resp.status(), StatusCode::OK);
            let updated_node: Node = serde_yaml::from_slice(&test::read_body(resp)).unwrap();
            assert_eq!(updated_node, node);

            // Verify that attempting to change the node identity gets a FORBIDDEN response
            let old_identity = node.identity.clone();
            node.identity = "Node-789".into();

            let req = test::TestRequest::patch()
                .uri(&format!("/admin/nodes/{}", &old_identity))
                .header(header::CONTENT_TYPE, "application/json")
                .set_json(&node)
                .to_request();

            let resp = test::call_service(&mut app, req);

            assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        })
    }

    #[test]
    /// Test the DELETE /admin/nodes/{identity} route for deleting a node from the registry.
    fn test_delete_node() {
        run_test(|test_yaml_file_path| {
            write_to_file(&test_yaml_file_path, &[get_node_1()]);

            let node_registry = new_yaml_node_registry(test_yaml_file_path);

            let mut app = test::init_service(
                App::new().data(node_registry.clone()).service(
                    web::resource("/admin/nodes/{identity}")
                        .route(web::delete().to_async(delete_node::<LocalYamlNodeRegistry>)),
                ),
            );

            // Verify that an existing node gets an OK response
            let req = test::TestRequest::delete()
                .uri(&format!("/admin/nodes/{}", get_node_1().identity))
                .to_request();

            let resp = test::call_service(&mut app, req);

            assert_eq!(resp.status(), StatusCode::OK);

            // Verify that a non-existent node gets a NOT_FOUND response
            let req = test::TestRequest::delete()
                .uri(&format!("/admin/nodes/{}", get_node_1().identity))
                .to_request();

            let resp = test::call_service(&mut app, req);

            assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        })
    }

    fn write_to_file(file_path: &str, nodes: &[Node]) {
        let file = File::create(file_path).expect("Error creating test nodes yaml file.");
        serde_yaml::to_writer(file, nodes).expect("Error writing nodes to file.");
    }

    fn get_node_1() -> Node {
        NodeBuilder::new("Node-123")
            .with_endpoint("12.0.0.123:8431")
            .with_display_name("Bitwise IO - Node 1")
            .with_metadata("company", "Bitwise IO")
            .build()
            .expect("Failed to build node1")
    }

    fn get_node_2() -> Node {
        NodeBuilder::new("Node-456")
            .with_endpoint("13.0.0.123:8434")
            .with_display_name("Cargill - Node 1")
            .with_metadata("company", "Cargill")
            .build()
            .expect("Failed to build node2")
    }

    fn run_test<T>(test: T) -> ()
    where
        T: FnOnce(&str) -> () + panic::UnwindSafe,
    {
        let test_yaml_file = temp_yaml_file_path();

        let test_path = test_yaml_file.clone();
        let result = panic::catch_unwind(move || test(&test_path));

        remove_file(test_yaml_file).unwrap();

        assert!(result.is_ok())
    }

    fn temp_yaml_file_path() -> String {
        let mut temp_dir = env::temp_dir();

        let thread_id = thread::current().id();
        temp_dir.push(format!("test_node_endpoint-{:?}.yaml", thread_id));
        temp_dir.to_str().unwrap().to_string()
    }
}
