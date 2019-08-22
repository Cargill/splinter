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

use super::{Paging, DEFAULT_LIMIT, DEFAULT_OFFSET, QUERY_ENCODE_SET};
use actix_web::{client::Client, http::StatusCode, web, Error, HttpResponse};
use futures::Future;
use libsplinter::node_registry::Node;
use percent_encoding::utf8_percent_encode;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ListNodesResponse {
    pub data: Vec<Node>,
    pub paging: Paging,
}

pub fn fetch_node(
    identity: web::Path<String>,
    client: web::Data<(Client, String)>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    let splinterd_url = &client.1;
    let client = &client.0;

    client
        .get(&format!("{}/nodes/{}", splinterd_url, identity))
        .send()
        .map_err(Error::from)
        .and_then(|mut resp| {
            let body = resp.body().wait()?;
            match resp.status() {
                StatusCode::OK => {
                    let node: Node = serde_json::from_slice(&body)?;
                    Ok(HttpResponse::Ok().json(node))
                }
                StatusCode::NOT_FOUND => {
                    let message: String = serde_json::from_slice(&body)?;
                    Ok(HttpResponse::NotFound().json(message))
                }
                _ => {
                    let message: String = serde_json::from_slice(&body)?;
                    Ok(HttpResponse::InternalServerError().json(message))
                }
            }
        })
}

pub fn list_nodes(
    client: web::Data<(Client, String)>,
    query: web::Query<HashMap<String, String>>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    let splinterd_url = &client.1;
    let client = &client.0;

    let mut request_url = format!("{}/nodes", splinterd_url);

    let offset = query
        .get("offset")
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| DEFAULT_OFFSET.to_string());
    let limit = query
        .get("limit")
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| DEFAULT_LIMIT.to_string());

    request_url = format!("{}?offset={}&limit={}", request_url, offset, limit);

    if let Some(filter) = query.get("filter") {
        request_url = format!(
            "{}&filter={}",
            request_url,
            utf8_percent_encode(filter, QUERY_ENCODE_SET).to_string()
        );
    }

    client
        .get(&request_url)
        .send()
        .map_err(Error::from)
        .and_then(|mut resp| {
            let body = resp.body().wait()?;
            match resp.status() {
                StatusCode::OK => {
                    let nodes: ListNodesResponse = serde_json::from_slice(&body)?;
                    Ok(HttpResponse::Ok().json(nodes))
                }
                StatusCode::BAD_REQUEST => {
                    let message: String = serde_json::from_slice(&body)?;
                    Ok(HttpResponse::BadRequest().json(message))
                }
                _ => {
                    let message: String = serde_json::from_slice(&body)?;
                    Ok(HttpResponse::InternalServerError().json(message))
                }
            }
        })
}

#[cfg(all(feature = "test-node-endpoint", test))]
mod test {
    use super::*;
    use actix_web::{
        http::{header, StatusCode},
        test, web, App,
    };

    static SPLINTERD_URL: &str = "http://splinterd-node:8085";

    #[test]
    /// Tests a GET /nodes/{identity} request returns the expected node.
    fn test_fetch_node_ok() {
        let mut app = test::init_service(
            App::new()
                .data((Client::new(), SPLINTERD_URL.to_string()))
                .service(web::resource("/nodes/{identity}").route(web::get().to_async(fetch_node))),
        );

        let req = test::TestRequest::get()
            .uri(&format!("/nodes/{}", get_node_1().identity))
            .to_request();

        let resp = test::call_service(&mut app, req);

        assert_eq!(resp.status(), StatusCode::OK);
        let node: Node = serde_json::from_slice(&test::read_body(resp)).unwrap();
        assert_eq!(node, get_node_1())
    }

    #[test]
    /// Tests a GET /nodes/{identity} request returns NotFound when an invalid identity is passed
    fn test_fetch_node_not_found() {
        let mut app = test::init_service(
            App::new()
                .data((Client::new(), SPLINTERD_URL.to_string()))
                .service(web::resource("/nodes/{identity}").route(web::get().to_async(fetch_node))),
        );

        let req = test::TestRequest::get()
            .uri("/nodes/Node-not-valid")
            .to_request();

        let resp = test::call_service(&mut app, req);

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    /// Tests a GET /nodes request with no filters returns the expected nodes.
    fn test_list_node_ok() {
        let mut app = test::init_service(
            App::new()
                .data((Client::new(), SPLINTERD_URL.to_string()))
                .service(web::resource("/nodes").route(web::get().to_async(list_nodes))),
        );

        let req = test::TestRequest::get().uri("/nodes").to_request();

        let resp = test::call_service(&mut app, req);

        assert_eq!(resp.status(), StatusCode::OK);
        let nodes: ListNodesResponse = serde_json::from_slice(&test::read_body(resp)).unwrap();
        assert_eq!(nodes.data, vec![get_node_1(), get_node_2()]);
        assert_eq!(
            nodes.paging,
            create_test_paging_response(0, 100, 0, 0, 0, 2, "/nodes?")
        )
    }

    #[test]
    /// Tests a GET /nodes request with filters returns the expected node.
    fn test_list_node_with_filters_ok() {
        let mut app = test::init_service(
            App::new()
                .data((Client::new(), SPLINTERD_URL.to_string()))
                .service(web::resource("/nodes").route(web::get().to_async(list_nodes))),
        );

        let filter = utf8_percent_encode("{\"company\":[\"=\",\"Bitwise IO\"]}", QUERY_ENCODE_SET)
            .to_string();

        let req = test::TestRequest::get()
            .uri(&format!("/nodes?filter={}", filter))
            .header(header::CONTENT_TYPE, "application/json")
            .to_request();

        let resp = test::call_service(&mut app, req);

        assert_eq!(resp.status(), StatusCode::OK);
        let nodes: ListNodesResponse = serde_json::from_slice(&test::read_body(resp)).unwrap();
        assert_eq!(nodes.data, vec![get_node_1()]);
        let link = format!("/nodes?filter={}&", filter);
        assert_eq!(
            nodes.paging,
            create_test_paging_response(0, 100, 0, 0, 0, 1, &link)
        )
    }

    #[test]
    /// Tests a GET /nodes request with invalid filter returns BadRequest response.
    fn test_list_node_with_filters_bad_request() {
        let mut app = test::init_service(
            App::new()
                .data((Client::new(), SPLINTERD_URL.to_string()))
                .service(web::resource("/nodes").route(web::get().to_async(list_nodes))),
        );

        let filter = utf8_percent_encode("{\"company\":[\"*\",\"Bitwise IO\"]}", QUERY_ENCODE_SET)
            .to_string();

        let req = test::TestRequest::get()
            .uri(&format!("/nodes?filter={}", filter))
            .header(header::CONTENT_TYPE, "application/json")
            .to_request();

        let resp = test::call_service(&mut app, req);

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    fn get_node_1() -> Node {
        let mut metadata = HashMap::new();
        metadata.insert("url".to_string(), "127.0.0.1:8080".to_string());
        metadata.insert("company".to_string(), "Bitwise IO".to_string());
        Node {
            identity: "Node-123".to_string(),
            metadata,
        }
    }

    fn get_node_2() -> Node {
        let mut metadata = HashMap::new();
        metadata.insert("url".to_string(), "127.0.0.1:8082".to_string());
        metadata.insert("company".to_string(), "Cargill".to_string());
        Node {
            identity: "Node-456".to_string(),
            metadata,
        }
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

}
