// Copyright 2019 Cargill Incorporated
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

use super::{get_response_paging_info, Paging, DEFAULT_LIMIT, DEFAULT_OFFSET};
use actix_web::{error::BlockingError, web, Error, HttpRequest, HttpResponse};
use futures::{future::IntoFuture, Future};
use libsplinter::node_registry::{error::NodeRegistryError, Node, NodeRegistry};
use std::collections::HashMap;
use url::percent_encoding::{utf8_percent_encode, QUERY_ENCODE_SET};

type Filter = HashMap<String, (String, String)>;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ListNodesResponse {
    data: Vec<Node>,
    paging: Paging,
}

pub fn fetch_node(
    identity: web::Path<String>,
    registry: web::Data<Box<dyn NodeRegistry>>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    web::block(move || registry.fetch_node(&identity.into_inner())).then(|res| match res {
        Ok(node) => Ok(HttpResponse::Ok().json(node)),
        Err(err) => match err {
            BlockingError::Error(err) => match err {
                NodeRegistryError::NotFoundError(err) => Ok(HttpResponse::NotFound().json(err)),
                _ => Ok(HttpResponse::InternalServerError().json(format!("{}", err))),
            },
            _ => Ok(HttpResponse::InternalServerError().json(format!("{}", err))),
        },
    })
}

pub fn list_nodes(
    req: HttpRequest,
    registry: web::Data<Box<dyn NodeRegistry>>,
    query: web::Query<HashMap<String, String>>,
) -> Box<Future<Item = HttpResponse, Error = Error>> {
    let offset = match query.get("offset") {
        Some(value) => match value.parse::<usize>() {
            Ok(val) => val,
            Err(err) => {
                return Box::new(
                    HttpResponse::BadRequest()
                        .json(format!(
                            "Invalid offset value passed: {}. Error: {}",
                            value, err
                        ))
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
                        .json(format!(
                            "Invalid limit value passed: {}. Error: {}",
                            value, err
                        ))
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
                link.push_str(&format!(
                    "filter={}&",
                    utf8_percent_encode(value, QUERY_ENCODE_SET).to_string()
                ));
                Some(val)
            }
            Err(err) => {
                return Box::new(
                    HttpResponse::BadRequest()
                        .json(format!(
                            "Invalid filter value passed: {}. Error: {}",
                            value, err
                        ))
                        .into_future(),
                )
            }
        },
        None => None,
    };

    Box::new(query_list_nodes(
        registry,
        link,
        filters,
        Some(offset),
        Some(limit),
    ))
}

fn query_list_nodes(
    registry: web::Data<Box<dyn NodeRegistry>>,
    link: String,
    filters: Option<Filter>,
    offset: Option<usize>,
    limit: Option<usize>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    web::block(
        move || match registry.list_nodes(filters.clone(), None, None) {
            Ok(nodes) => Ok((registry, filters, nodes.len(), link, limit, offset)),
            Err(err) => Err(err),
        },
    )
    .and_then(|(registry, filters, total_count, link, limit, offset)| {
        web::block(move || match registry.list_nodes(filters, limit, offset) {
            Ok(nodes) => Ok((nodes, link, limit, offset, total_count)),
            Err(err) => Err(err),
        })
    })
    .then(|res| match res {
        Ok((nodes, link, limit, offset, total_count)) => {
            Ok(HttpResponse::Ok().json(ListNodesResponse {
                data: nodes,
                paging: get_response_paging_info(limit, offset, &link, total_count),
            }))
        }
        Err(err) => match err {
            BlockingError::Error(err) => match err {
                NodeRegistryError::InvalidFilterError(err) => {
                    Ok(HttpResponse::BadRequest().json(err))
                }
                _ => Ok(HttpResponse::InternalServerError().into()),
            },
            _ => Ok(HttpResponse::InternalServerError().into()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_registry::yaml::YamlNodeRegistry;
    use libsplinter::node_registry::Node;
    use std::collections::HashMap;
    use std::env;
    use std::fs::{remove_file, File};
    use std::panic;
    use std::thread;

    use actix_web::{
        http::{header, StatusCode},
        test, web, App,
    };

    #[test]
    /// Tests a GET /node/{identity} request returns the expected node.
    fn test_fetch_node_ok() {
        run_test(|test_yaml_file_path| {
            write_to_file(&test_yaml_file_path);

            let node_registry: Box<dyn NodeRegistry> = Box::new(
                YamlNodeRegistry::new(test_yaml_file_path)
                    .expect("Error creating YamlNodeRegistry"),
            );

            let mut app =
                test::init_service(App::new().data(node_registry.clone()).service(
                    web::resource("/node/{identity}").route(web::get().to_async(fetch_node)),
                ));

            let req = test::TestRequest::get()
                .uri(&format!("/node/{}", get_node_1().identity))
                .to_request();

            let resp = test::call_service(&mut app, req);

            assert_eq!(resp.status(), StatusCode::OK);
            let node: Node = serde_yaml::from_slice(&test::read_body(resp)).unwrap();
            assert_eq!(node, get_node_1())
        })
    }

    #[test]
    /// Tests a GET /node/{identity} request returns NotFound when an invalid identity is passed
    fn test_fetch_node_not_found() {
        run_test(|test_yaml_file_path| {
            write_to_file(&test_yaml_file_path);

            let node_registry: Box<dyn NodeRegistry> = Box::new(
                YamlNodeRegistry::new(test_yaml_file_path)
                    .expect("Error creating YamlNodeRegistry"),
            );

            let mut app =
                test::init_service(App::new().data(node_registry.clone()).service(
                    web::resource("/node/{identity}").route(web::get().to_async(fetch_node)),
                ));

            let req = test::TestRequest::get()
                .uri("/node/Node-not-valid")
                .to_request();

            let resp = test::call_service(&mut app, req);

            assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        })
    }

    #[test]
    /// Tests a GET /node request with no filters returns the expected nodes.
    fn test_list_node_ok() {
        run_test(|test_yaml_file_path| {
            write_to_file(&test_yaml_file_path);

            let node_registry: Box<dyn NodeRegistry> = Box::new(
                YamlNodeRegistry::new(test_yaml_file_path)
                    .expect("Error creating YamlNodeRegistry"),
            );

            let mut app = test::init_service(
                App::new()
                    .data(node_registry.clone())
                    .service(web::resource("/node").route(web::get().to_async(list_nodes))),
            );

            let req = test::TestRequest::get().uri("/node").to_request();

            let resp = test::call_service(&mut app, req);

            assert_eq!(resp.status(), StatusCode::OK);
            let nodes: ListNodesResponse = serde_yaml::from_slice(&test::read_body(resp)).unwrap();
            assert_eq!(nodes.data, vec![get_node_1(), get_node_2()]);
            assert_eq!(
                nodes.paging,
                create_test_paging_response(0, 100, 0, 0, 0, 2, "/node?")
            )
        })
    }

    #[test]
    /// Tests a GET /node request with filters returns the expected node.
    fn test_list_node_with_filters_ok() {
        run_test(|test_yaml_file_path| {
            write_to_file(&test_yaml_file_path);

            let node_registry: Box<dyn NodeRegistry> = Box::new(
                YamlNodeRegistry::new(test_yaml_file_path)
                    .expect("Error creating YamlNodeRegistry"),
            );

            let mut app = test::init_service(
                App::new()
                    .data(node_registry.clone())
                    .service(web::resource("/node").route(web::get().to_async(list_nodes))),
            );

            let filter =
                utf8_percent_encode("{\"company\":[\"=\",\"Bitwise IO\"]}", QUERY_ENCODE_SET)
                    .to_string();

            let req = test::TestRequest::get()
                .uri(&format!("/node?filter={}", filter))
                .header(header::CONTENT_TYPE, "application/json")
                .to_request();

            let resp = test::call_service(&mut app, req);

            assert_eq!(resp.status(), StatusCode::OK);
            let nodes: ListNodesResponse = serde_yaml::from_slice(&test::read_body(resp)).unwrap();
            assert_eq!(nodes.data, vec![get_node_1()]);
            let link = format!("/node?filter={}&", filter);
            assert_eq!(
                nodes.paging,
                create_test_paging_response(0, 100, 0, 0, 0, 1, &link)
            )
        })
    }

    #[test]
    /// Tests a GET /node request with invalid filter returns BadRequest response.
    fn test_list_node_with_filters_bad_request() {
        run_test(|test_yaml_file_path| {
            write_to_file(&test_yaml_file_path);

            let node_registry: Box<dyn NodeRegistry> = Box::new(
                YamlNodeRegistry::new(test_yaml_file_path)
                    .expect("Error creating YamlNodeRegistry"),
            );

            let mut app = test::init_service(
                App::new()
                    .data(node_registry.clone())
                    .service(web::resource("/node").route(web::get().to_async(list_nodes))),
            );

            let filter =
                utf8_percent_encode("{\"company\":[\"*\",\"Bitwise IO\"]}", QUERY_ENCODE_SET)
                    .to_string();

            let req = test::TestRequest::get()
                .uri(&format!("/node?filter={}", filter))
                .header(header::CONTENT_TYPE, "application/json")
                .to_request();

            let resp = test::call_service(&mut app, req);

            assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        })
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

    fn write_to_file(file_path: &str) {
        let file = File::create(file_path).expect("Error creating test nodes yaml file.");
        serde_yaml::to_writer(file, &vec![get_node_1(), get_node_2()])
            .expect("Error writing nodes to file.");
    }

    fn get_node_1() -> Node {
        let mut metadata = HashMap::new();
        metadata.insert("url".to_string(), "12.0.0.123:8431".to_string());
        metadata.insert("company".to_string(), "Bitwise IO".to_string());
        Node {
            identity: "Node-123".to_string(),
            metadata,
        }
    }

    fn get_node_2() -> Node {
        let mut metadata = HashMap::new();
        metadata.insert("url".to_string(), "13.0.0.123:8434".to_string());
        metadata.insert("company".to_string(), "Cargill".to_string());
        Node {
            identity: "Node-456".to_string(),
            metadata,
        }
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
