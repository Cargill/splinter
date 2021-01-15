// Copyright 2018-2021 Cargill Incorporated
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

use actix_web::{client::Client, http::StatusCode, web, Error, HttpResponse};
use percent_encoding::utf8_percent_encode;
use splinter::protocol;
use std::collections::HashMap;

use super::{ErrorResponse, SuccessResponse, DEFAULT_LIMIT, DEFAULT_OFFSET, QUERY_ENCODE_SET};

pub async fn fetch_node(
    identity: web::Path<String>,
    client: web::Data<Client>,
    splinterd_url: web::Data<String>,
) -> Result<HttpResponse, Error> {
    let mut response = client
        .get(&format!(
            "{}/registry/nodes/{}",
            splinterd_url.get_ref(),
            identity
        ))
        .header(
            "SplinterProtocolVersion",
            protocol::REGISTRY_PROTOCOL_VERSION.to_string(),
        )
        .send()
        .await?;

    let body = response.body().await?;

    match response.status() {
        StatusCode::OK => {
            let node: NodeResponse = serde_json::from_slice(&body)?;
            Ok(HttpResponse::Ok().json(SuccessResponse::new(node)))
        }
        StatusCode::NOT_FOUND => {
            let err_response: SplinterdErrorResponse = serde_json::from_slice(&body)?;
            Ok(HttpResponse::NotFound().json(ErrorResponse::not_found(&err_response.message)))
        }
        _ => {
            let err_response: SplinterdErrorResponse = serde_json::from_slice(&body)?;
            debug!(
                "Internal Server Error. Splinterd responded with error {} message {}",
                response.status(),
                err_response.message
            );
            Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()))
        }
    }
}

pub async fn list_nodes(
    client: web::Data<Client>,
    splinterd_url: web::Data<String>,
    query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse, Error> {
    let mut request_url = format!("{}/registry/nodes", splinterd_url.get_ref());

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

    let mut response = client
        .get(&request_url)
        .header(
            "SplinterProtocolVersion",
            protocol::REGISTRY_PROTOCOL_VERSION.to_string(),
        )
        .send()
        .await?;

    let body = response.body().await?;

    match response.status() {
        StatusCode::OK => {
            let list_reponse: SuccessResponse<Vec<NodeResponse>> = serde_json::from_slice(&body)?;
            Ok(HttpResponse::Ok().json(list_reponse))
        }
        StatusCode::BAD_REQUEST => {
            let err_response: SplinterdErrorResponse = serde_json::from_slice(&body)?;
            Ok(HttpResponse::BadRequest().json(ErrorResponse::bad_request(&err_response.message)))
        }
        _ => {
            let err_response: SplinterdErrorResponse = serde_json::from_slice(&body)?;
            debug!(
                "Internal Server Error. Splinterd responded with error {} message {}",
                response.status(),
                err_response.message
            );
            Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()))
        }
    }
}

/// Represents a node as presented by the Splinter REST API.
#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct NodeResponse {
    identity: String,
    endpoints: Vec<String>,
    display_name: String,
    metadata: HashMap<String, String>,
}

#[derive(Deserialize)]
struct SplinterdErrorResponse {
    message: String,
}

#[cfg(all(feature = "test-node-endpoint", test))]
mod test {
    use super::*;
    use crate::rest_api::routes::Paging;
    use actix_web::{
        http::{header, StatusCode},
        test, web, App,
    };

    static SPLINTERD_URL: &str = "http://splinterd-node:8085";

    #[actix_rt::test]
    /// Tests a GET /registry/nodes/{identity} request returns the expected node.
    async fn test_fetch_node_ok() {
        let mut app = test::init_service(
            App::new()
                .data(Client::new())
                .data(SPLINTERD_URL.to_string())
                .service(
                    web::resource("/registry/nodes/{identity}").route(web::get().to(fetch_node)),
                ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/registry/nodes/{}", get_node_1().identity))
            .to_request();

        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
        let response: SuccessResponse<NodeResponse> =
            serde_json::from_slice(&test::read_body(resp).await).unwrap();
        assert_eq!(response.data, get_node_1())
    }

    #[actix_rt::test]
    /// Tests a GET /registry/nodes/{identity} request returns NotFound when an invalid identity is passed
    async fn test_fetch_node_not_found() {
        let mut app = test::init_service(
            App::new()
                .data(Client::new())
                .data(SPLINTERD_URL.to_string())
                .service(
                    web::resource("/registry/nodes/{identity}").route(web::get().to(fetch_node)),
                ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/registry/nodes/Node-not-valid")
            .to_request();

        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[actix_rt::test]
    /// Tests a GET /registry/nodes request with no filters returns the expected nodes.
    async fn test_list_node_ok() {
        let mut app = test::init_service(
            App::new()
                .data(Client::new())
                .data(SPLINTERD_URL.to_string())
                .service(web::resource("/registry/nodes").route(web::get().to(list_nodes))),
        )
        .await;

        let req = test::TestRequest::get().uri("/registry/nodes").to_request();

        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
        let nodes: SuccessResponse<Vec<NodeResponse>> =
            serde_json::from_slice(&test::read_body(resp).await).unwrap();
        assert_eq!(nodes.data.len(), 2);
        assert!(nodes.data.contains(&get_node_1()));
        assert!(nodes.data.contains(&get_node_2()));
        assert_eq!(
            nodes.paging,
            Some(create_test_paging_response(
                0,
                100,
                0,
                0,
                0,
                2,
                "/registry/nodes?"
            ))
        )
    }

    #[actix_rt::test]
    /// Tests a GET /registry/nodes request with filters returns the expected node.
    async fn test_list_node_with_filters_ok() {
        let mut app = test::init_service(
            App::new()
                .data(Client::new())
                .data(SPLINTERD_URL.to_string())
                .service(web::resource("/registry/nodes").route(web::get().to(list_nodes))),
        )
        .await;

        let filter = utf8_percent_encode("{\"company\":[\"=\",\"Bitwise IO\"]}", QUERY_ENCODE_SET)
            .to_string();

        let req = test::TestRequest::get()
            .uri(&format!("/registry/nodes?filter={}", filter))
            .header(header::CONTENT_TYPE, "application/json")
            .to_request();

        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
        let nodes: SuccessResponse<Vec<NodeResponse>> =
            serde_json::from_slice(&test::read_body(resp).await).unwrap();
        assert_eq!(nodes.data, vec![get_node_1()]);
        let link = format!("/registry/nodes?filter={}&", filter);
        assert_eq!(
            nodes.paging,
            Some(create_test_paging_response(0, 100, 0, 0, 0, 1, &link))
        )
    }

    #[actix_rt::test]
    /// Tests a GET /registry/nodes request with invalid filter returns BadRequest response.
    async fn test_list_node_with_filters_bad_request() {
        let mut app = test::init_service(
            App::new()
                .data(Client::new())
                .data(SPLINTERD_URL.to_string())
                .service(web::resource("/registry/nodes").route(web::get().to(list_nodes))),
        )
        .await;

        let filter = utf8_percent_encode("{\"company\":[\"*\",\"Bitwise IO\"]}", QUERY_ENCODE_SET)
            .to_string();

        let req = test::TestRequest::get()
            .uri(&format!("/registry/nodes?filter={}", filter))
            .header(header::CONTENT_TYPE, "application/json")
            .to_request();

        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    fn get_node_1() -> NodeResponse {
        let mut metadata = HashMap::new();
        metadata.insert("company".into(), "Bitwise IO".into());
        NodeResponse {
            identity: "Node-123".into(),
            endpoints: vec!["tcps://127.0.0.1:8080".into()],
            display_name: "Bitwise IO - Node 1".into(),
            metadata,
        }
    }

    fn get_node_2() -> NodeResponse {
        let mut metadata = HashMap::new();
        metadata.insert("company".into(), "Cargill".into());
        NodeResponse {
            identity: "Node-456".into(),
            endpoints: vec!["tcps://127.0.0.1:8082".into()],
            display_name: "Cargill - Node 1".into(),
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
