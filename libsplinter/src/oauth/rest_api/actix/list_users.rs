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

use crate::actix_web::{web, HttpResponse};
use crate::biome::oauth::store::OAuthUserSessionStore;
use crate::oauth::rest_api::{
    resources::list_users::{ListOAuthUserResponse, OAuthUserResponse, PagingQuery},
    OAUTH_USER_READ_PERMISSION,
};
use crate::protocol;
use crate::rest_api::{
    actix_web_1::{Method, ProtocolVersionRangeGuard, Resource},
    paging::get_response_paging_info,
    ErrorResponse,
};
use futures::future::IntoFuture;

pub fn make_oauth_list_users_resource(
    oauth_user_session_store: Box<dyn OAuthUserSessionStore>,
) -> Resource {
    let resource =
        Resource::build("/oauth/users").add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::OAUTH_USER_READ_PROTOCOL_MIN,
            protocol::OAUTH_PROTOCOL_VERSION,
        ));
    #[cfg(feature = "authorization")]
    {
        resource.add_method(Method::Get, OAUTH_USER_READ_PERMISSION, move |req, _| {
            let web::Query(paging_query): web::Query<PagingQuery> =
                match web::Query::from_query(req.query_string()) {
                    Ok(paging_query) => paging_query,
                    Err(_) => {
                        return Box::new(
                            HttpResponse::BadRequest()
                                .json(ErrorResponse::bad_request("Invalid query"))
                                .into_future(),
                        )
                    }
                };
            let link = format!("{}?", req.uri().path());
            Box::new(match oauth_user_session_store.list_users() {
                Ok(users) => {
                    let total = users.len();
                    let oauth_users = users
                        .skip(paging_query.offset)
                        .take(paging_query.limit)
                        .collect::<Vec<_>>();

                    HttpResponse::Ok()
                        .json(ListOAuthUserResponse {
                            data: oauth_users.iter().map(OAuthUserResponse::from).collect(),
                            paging: get_response_paging_info(
                                Some(paging_query.limit),
                                Some(paging_query.offset),
                                &link,
                                total,
                            ),
                        })
                        .into_future()
                }
                Err(err) => {
                    error!("Unable to remove user session: {}", err);
                    HttpResponse::InternalServerError()
                        .json(ErrorResponse::internal_error())
                        .into_future()
                }
            })
        })
    }
    #[cfg(not(feature = "authorization"))]
    {
        resource.add_method(Method::Get, move |req, _| {
            let web::Query(paging_query): web::Query<PagingQuery> =
                match web::Query::from_query(req.query_string()) {
                    Ok(paging_query) => paging_query,
                    Err(_) => {
                        return Box::new(
                            HttpResponse::BadRequest()
                                .json(ErrorResponse::bad_request("Invalid query"))
                                .into_future(),
                        )
                    }
                };
            let link = format!("{}?", req.uri().path());

            Box::new(match oauth_user_session_store.list_users() {
                Ok(users) => {
                    let total = users.total();
                    let oauth_users = users
                        .skip(paging_query.offset)
                        .take(paging_query.limit)
                        .collect::<Vec<_>>();
                    Ok(HttpResponse::Ok().json(ListOAuthUserResponse {
                        data: oauth_users.iter().map(OAuthUserResponse::from).collect(),
                        paging: get_response_paging_info(
                            Some(paging_query.limit),
                            Some(paging_query.offset),
                            &link,
                            total,
                        ),
                    }))
                }
                Err(err) => {
                    error!("Unable to remove user session: {}", err);
                    HttpResponse::InternalServerError()
                        .json(ErrorResponse::internal_error())
                        .into_future()
                }
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use reqwest::{blocking::Client, StatusCode, Url};

    use crate::biome::oauth::store::InsertableOAuthUserSessionBuilder;
    use crate::biome::MemoryOAuthUserSessionStore;
    use crate::rest_api::{
        actix_web_1::{RestApiBuilder, RestApiShutdownHandle},
        paging::Paging,
    };

    #[derive(Debug, Deserialize)]
    struct TestClientOAuthUser {
        subject: String,
        user_id: String,
    }

    #[derive(Deserialize)]
    struct TestClientOAuthUserListResponse {
        data: Vec<TestClientOAuthUser>,
        paging: Paging,
    }

    /// Tests a GET /oauth/users request which returns the list of users.
    /// 1. Adds two OAuth user sessions to the store
    /// 2. Perform a GET against /oauth/users
    /// 3. Verify that it includes both OAuth users
    #[test]
    fn test_list_oauth_users_ok() {
        let oauth_user_session_store = MemoryOAuthUserSessionStore::new();

        let splinter_access_token = "splinter_access_token";
        let subject = "subject_1";
        let oauth_access_token = "oauth_access_token";
        let session = InsertableOAuthUserSessionBuilder::new()
            .with_splinter_access_token(splinter_access_token.into())
            .with_subject(subject.into())
            .with_oauth_access_token(oauth_access_token.into())
            .build()
            .expect("Unable to build session");
        oauth_user_session_store
            .add_session(session)
            .expect("Unable to add session");

        let splinter_access_token2 = "splinter_access_token2";
        let subject2 = "subject_2";
        let oauth_access_token2 = "oauth_access_token2";
        let session = InsertableOAuthUserSessionBuilder::new()
            .with_splinter_access_token(splinter_access_token2.into())
            .with_subject(subject2.into())
            .with_oauth_access_token(oauth_access_token2.into())
            .build()
            .expect("Unable to build session");
        oauth_user_session_store
            .add_session(session)
            .expect("Unable to add session");

        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_oauth_list_users_resource(Box::new(
                oauth_user_session_store,
            ))]);

        let url =
            Url::parse(&format!("http://{}/oauth/users", bind_url)).expect("Failed to parse URL");

        let resp = Client::new()
            .get(url)
            .header(
                "SplinterProtocolVersion",
                protocol::AUTHORIZATION_PROTOCOL_VERSION,
            )
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let resp = resp
            .json::<TestClientOAuthUserListResponse>()
            .expect("Failed to deserialize body");
        let oauth_users = resp.data;
        assert_eq!(oauth_users.len(), 2);
        assert!(oauth_users.iter().any(|user| user.subject == subject));
        assert!(oauth_users.iter().any(|user| user.subject == subject2));

        let paging = resp.paging;

        assert_eq!(
            paging,
            create_test_paging_response(0, 100, 0, 0, 0, 2, "/oauth/users?")
        );

        shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    /// Tests a GET /oauth/users request returns the list of users according to the limit sent in
    /// the request.
    /// 1. Adds 101 OAuth user sessions to the store
    /// 2. Perform a GET against /oauth/users
    /// 3. Verify that 100 elements are initially returned, with a valid next URL
    /// 4. Perform a GET request against the next URL
    /// 5. Verify the final element is returned
    #[test]
    fn test_list_oauth_users_paging_ok() {
        let oauth_user_session_store = MemoryOAuthUserSessionStore::new();

        for i in 0..101 {
            let splinter_access_token = format!("splinter_access_token_{}", i);
            let subject = format!("subject_{}", i);
            let oauth_access_token = format!("oauth_access_token_{}", i);
            let session = InsertableOAuthUserSessionBuilder::new()
                .with_splinter_access_token(splinter_access_token.into())
                .with_subject(subject.into())
                .with_oauth_access_token(oauth_access_token.into())
                .build()
                .expect("Unable to build session");
            oauth_user_session_store
                .add_session(session)
                .expect("Unable to add session");
        }

        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_oauth_list_users_resource(Box::new(
                oauth_user_session_store,
            ))]);

        let url =
            Url::parse(&format!("http://{}/oauth/users", bind_url)).expect("Failed to parse URL");

        let resp = Client::new()
            .get(url)
            .header(
                "SplinterProtocolVersion",
                protocol::AUTHORIZATION_PROTOCOL_VERSION,
            )
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let resp = resp
            .json::<TestClientOAuthUserListResponse>()
            .expect("Failed to deserialize body");
        assert_eq!(resp.data.len(), 100);

        let paging = resp.paging;

        assert_eq!(
            paging,
            create_test_paging_response(0, 100, 100, 0, 100, 101, "/oauth/users?")
        );

        let next_link: String = paging.next;

        let url =
            Url::parse(&format!("http://{}{}", bind_url, next_link)).expect("Failed to parse URL");

        let resp = Client::new()
            .get(url)
            .header(
                "SplinterProtocolVersion",
                protocol::AUTHORIZATION_PROTOCOL_VERSION,
            )
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let resp = resp
            .json::<TestClientOAuthUserListResponse>()
            .expect("Failed to deserialize body");
        assert_eq!(resp.data.len(), 1);

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
        let bind = crate::rest_api::BindConfig::Http("127.0.0.1:0".into());

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
