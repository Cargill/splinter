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

//! This module provides the following endpoints:
//!
//! * `GET /authorization/roles` for listing roles

use std::convert::TryInto;

use crate::actix_web::{error::BlockingError, web, Error, HttpRequest, HttpResponse};
use crate::error::InvalidStateError;
use crate::futures::{stream::Stream, Future, IntoFuture};
use crate::protocol;
use crate::rest_api::{
    actix_web_1::{Method, ProtocolVersionRangeGuard, Resource},
    auth::rbac::{
        rest_api::{
            resources::roles::{ListRoleResponse, RolePayload, RoleResponse},
            RBAC_READ_PERMISSION, RBAC_WRITE_PERMISSION,
        },
        store::{Role, RoleBasedAuthorizationStore},
    },
    paging::{get_response_paging_info, DEFAULT_LIMIT, DEFAULT_OFFSET},
    ErrorResponse,
};

use super::error::SendableRoleBasedAuthorizationStoreError;

#[derive(Deserialize)]
struct PagingQuery {
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default = "default_offset")]
    offset: usize,
}

fn default_limit() -> usize {
    DEFAULT_LIMIT
}

fn default_offset() -> usize {
    DEFAULT_OFFSET
}

pub fn make_roles_resource(
    role_based_authorization_store: Box<dyn RoleBasedAuthorizationStore>,
) -> Resource {
    let list_store = role_based_authorization_store.clone();
    let post_store = role_based_authorization_store;
    Resource::build("/authorization/roles")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::AUTHORIZATION_RBAC_ROLES_MIN,
            protocol::AUTHORIZATION_PROTOCOL_VERSION,
        ))
        .add_method(Method::Get, RBAC_READ_PERMISSION, move |r, _| {
            list_roles(r, web::Data::new(list_store.clone()))
        })
        .add_method(Method::Post, RBAC_WRITE_PERMISSION, move |_, p| {
            add_role(p, web::Data::new(post_store.clone()))
        })
}

fn list_roles(
    req: HttpRequest,
    role_based_authorization_store: web::Data<Box<dyn RoleBasedAuthorizationStore>>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
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

    Box::new(
        web::block(move || {
            let roles = role_based_authorization_store
                .list_roles()
                .map_err(SendableRoleBasedAuthorizationStoreError::from)?;

            let total = roles.len();
            let roles = roles
                .skip(paging_query.offset)
                .take(paging_query.limit)
                .collect::<Vec<_>>();

            Ok((roles, link, paging_query, total))
        })
        .then(
            |res: Result<_, BlockingError<SendableRoleBasedAuthorizationStoreError>>| match res {
                Ok((roles, link, paging_query, total)) => {
                    Ok(HttpResponse::Ok().json(ListRoleResponse {
                        data: roles.iter().map(RoleResponse::from).collect(),
                        paging: get_response_paging_info(
                            Some(paging_query.limit),
                            Some(paging_query.offset),
                            &link,
                            total,
                        ),
                    }))
                }
                Err(err) => {
                    error!("Unable to list roles: {}", err);
                    Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()))
                }
            },
        ),
    )
}

fn add_role(
    payload: web::Payload,
    role_based_authorization_store: web::Data<Box<dyn RoleBasedAuthorizationStore>>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    Box::new(
        payload
            .from_err::<Error>()
            .fold(web::BytesMut::new(), move |mut body, chunk| {
                body.extend_from_slice(&chunk);
                Ok::<_, Error>(body)
            })
            .into_future()
            .and_then(move |body| {
                let role_res: Result<Role, _> = serde_json::from_slice::<RolePayload>(&body)
                    .map_err(|err| err.to_string())
                    .and_then(|role_payload| {
                        role_payload
                            .try_into()
                            .map_err(|err: InvalidStateError| err.to_string())
                    });

                match role_res {
                    Ok(role) => Box::new(
                        web::block(move || {
                            role_based_authorization_store
                                .add_role(role)
                                .map_err(SendableRoleBasedAuthorizationStoreError::from)
                        })
                        .then(|res| {
                            Ok(match res {
                                Ok(_) => HttpResponse::Ok().finish(),
                                Err(BlockingError::Error(
                                    SendableRoleBasedAuthorizationStoreError::ConstraintViolation(
                                        msg,
                                    ),
                                )) => HttpResponse::Conflict().json(ErrorResponse::conflict(&msg)),
                                Err(err) => {
                                    error!("Unable to add role: {}", err);
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
                                "Invalid role payload: {}",
                                err
                            )))
                            .into_future(),
                    ),
                }
            }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    use reqwest::{blocking::Client, StatusCode, Url};
    use serde_json::{to_value, Value as JsonValue};

    use crate::error::{ConstraintViolationError, ConstraintViolationType};
    use crate::rest_api::auth::rbac::store::{
        Assignment, Identity, Role, RoleBasedAuthorizationStoreError, RoleBuilder,
    };
    use crate::rest_api::{
        actix_web_1::{RestApiBuilder, RestApiShutdownHandle},
        paging::Paging,
    };

    macro_rules! get_in {
        ($val:expr, $keys:expr, $as:ident) => {{
            let mut result = Some(&$val);
            for k in $keys {
                result = result.and_then(|next| next.get(k));
            }

            result.and_then(|last_val| last_val.$as())
        }};
    }

    /// Tests a GET /authorization/roles request which returns the set of nodes in the registry.
    #[test]
    fn test_list_roles_ok() {
        let role_based_auth_store = MemRoleBasedAuthorizationStore::default();

        let role = RoleBuilder::new()
            .with_id("test-role-1".into())
            .with_display_name("Test Role 1".into())
            .with_permissions(vec!["a".to_string(), "b".to_string(), "c".to_string()])
            .build()
            .expect("Unable to build role");

        role_based_auth_store
            .add_role(role)
            .expect("Unable to add role");

        let role = RoleBuilder::new()
            .with_id("test-role-2".into())
            .with_display_name("Test Role 2".into())
            .with_permissions(vec!["x".to_string(), "y".to_string(), "z".to_string()])
            .build()
            .expect("Unable to build role");

        role_based_auth_store
            .add_role(role)
            .expect("Unable to add role");

        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_roles_resource(Box::new(role_based_auth_store))]);

        let url = Url::parse(&format!("http://{}/authorization/roles", bind_url))
            .expect("Failed to parse URL");

        let resp = Client::new()
            .get(url)
            .header(
                "SplinterProtocolVersion",
                protocol::AUTHORIZATION_PROTOCOL_VERSION,
            )
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let body: JsonValue = resp.json().expect("Failed to deserialize body");

        let json_roles = get_in!(body, &["data"], as_array)
            .expect("data field is not an array")
            .to_vec();

        assert_eq!(2, json_roles.len());

        assert_eq!(
            &to_value(RoleResponse {
                role_id: "test-role-1",
                display_name: "Test Role 1",
                permissions: &["a".to_string(), "b".to_string(), "c".to_string()],
            })
            .expect("Failed to convert to value"),
            json_roles.get(0).expect("no first item")
        );
        assert_eq!(
            &to_value(RoleResponse {
                role_id: "test-role-2",
                display_name: "Test Role 2",
                permissions: &["x".to_string(), "y".to_string(), "z".to_string()],
            })
            .expect("Failed to convert to value"),
            json_roles.get(1).expect("no first item")
        );

        assert_eq!(
            body.get("paging").expect("no paging field in response"),
            &to_value(create_test_paging_response(
                0,
                100,
                0,
                0,
                0,
                2,
                "/authorization/roles?"
            ))
            .expect("failed to convert expected paging")
        );

        shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    /// Tests a GET /authorization/roles request which returns the paged elements.  It fetches the
    /// items on the second page and validates that the correct number are there.
    #[test]
    fn test_list_roles_paging_ok() {
        let role_based_auth_store = MemRoleBasedAuthorizationStore::default();

        for i in 0..101 {
            let role = RoleBuilder::new()
                .with_id(format!("test-role-{:0>3}", i))
                .with_display_name(format!("Test Role {}", i))
                .with_permissions(vec![format!("perm-{}", i)])
                .build()
                .expect("Unable to build role");

            role_based_auth_store
                .add_role(role)
                .expect("Unable to add role");
        }

        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_roles_resource(Box::new(role_based_auth_store))]);

        let url = Url::parse(&format!("http://{}/authorization/roles", bind_url))
            .expect("Failed to parse URL");

        let resp = Client::new()
            .get(url)
            .header(
                "SplinterProtocolVersion",
                protocol::AUTHORIZATION_PROTOCOL_VERSION,
            )
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let body: JsonValue = resp.json().expect("Failed to deserialize body");

        let json_roles = get_in!(body, &["data"], as_array)
            .expect("data field is not an array")
            .to_vec();
        assert_eq!(100, json_roles.len());

        for i in 0..100 {
            assert_eq!(
                &to_value(RoleResponse {
                    role_id: &format!("test-role-{:0>3}", i),
                    display_name: &format!("Test Role {}", i),
                    permissions: &[format!("perm-{}", i)],
                })
                .expect("Failed to convert to value"),
                json_roles.get(i).expect("no first item")
            );
        }

        assert_eq!(
            &to_value(create_test_paging_response(
                0,
                100,
                100,
                0,
                100,
                101,
                "/authorization/roles?"
            ))
            .expect("failed to convert expected paging"),
            body.get("paging").expect("no paging field in response"),
        );

        let next_link = get_in!(body, &["paging", "next"], as_str)
            .expect("paging.next field should be a string");

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
        let body: JsonValue = resp.json().expect("Failed to deserialize body");

        let json_roles = body
            .get("data")
            .expect("No data field in response")
            .as_array()
            .expect("data field is not an array")
            .to_vec();
        assert_eq!(1, json_roles.len());

        assert_eq!(
            &to_value(RoleResponse {
                role_id: "test-role-100",
                display_name: "Test Role 100",
                permissions: &["perm-100".to_string()],
            })
            .expect("Failed to convert to value"),
            json_roles.get(0).expect("no first item")
        );

        shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    /// Tests a POST /authorization/roles request with a valid returns OK.
    /// Verify that the role has been added by querying the list of roles.
    #[test]
    fn test_post_role_ok() {
        let role_based_auth_store = MemRoleBasedAuthorizationStore::default();
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_roles_resource(Box::new(role_based_auth_store))]);

        let url = Url::parse(&format!("http://{}/authorization/roles", bind_url))
            .expect("Failed to parse URL");

        let resp = Client::new()
            .post(url.clone())
            .header(
                "SplinterProtocolVersion",
                protocol::AUTHORIZATION_PROTOCOL_VERSION,
            )
            .json(&json!({
                "role_id": "new_test_role",
                "display_name": "New Test Display Name",
                "permissions": ["my-permission-1", "my-permission-2"],
            }))
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);

        // verify the role is in the list
        let resp = Client::new()
            .get(url)
            .header(
                "SplinterProtocolVersion",
                protocol::AUTHORIZATION_PROTOCOL_VERSION,
            )
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let body: JsonValue = resp.json().expect("Failed to deserialize body");

        let json_roles = body
            .get("data")
            .expect("No data field in response")
            .as_array()
            .expect("data field is not an array")
            .to_vec();
        assert_eq!(1, json_roles.len());

        assert_eq!(
            &to_value(RoleResponse {
                role_id: "new_test_role",
                display_name: "New Test Display Name",
                permissions: &["my-permission-1".to_string(), "my-permission-2".to_string()],
            })
            .expect("Failed to convert to value"),
            json_roles.get(0).expect("no first item")
        );

        shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    /// Tests a POST /authorization/roles request with a valid returns OK.
    /// Verify that the role has been added by querying the list of roles.
    #[test]
    fn test_post_role_conflict() {
        let role_based_auth_store = MemRoleBasedAuthorizationStore::default();

        let role = RoleBuilder::new()
            .with_id("test-role-1".into())
            .with_display_name("Test Role 1".into())
            .with_permissions(vec!["x".to_string(), "y".to_string(), "z".to_string()])
            .build()
            .expect("Unable to build role");

        role_based_auth_store
            .add_role(role)
            .expect("Unable to add role");

        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_roles_resource(Box::new(role_based_auth_store))]);

        let url = Url::parse(&format!("http://{}/authorization/roles", bind_url))
            .expect("Failed to parse URL");

        let resp = Client::new()
            .post(url.clone())
            .header(
                "SplinterProtocolVersion",
                protocol::AUTHORIZATION_PROTOCOL_VERSION,
            )
            .json(&json!({
                "role_id": "test-role-1",
                "display_name": "Doesn't matter",
                "permissions": ["my-permission-1", "my-permission-2"],
            }))
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::CONFLICT);

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

    #[derive(Clone, Default)]
    struct MemRoleBasedAuthorizationStore {
        roles: Arc<Mutex<BTreeMap<String, Role>>>,
    }

    impl RoleBasedAuthorizationStore for MemRoleBasedAuthorizationStore {
        fn get_role(&self, id: &str) -> Result<Option<Role>, RoleBasedAuthorizationStoreError> {
            Ok(self
                .roles
                .lock()
                .expect("mem role based authorization store lock was poisoned")
                .get(id)
                .cloned())
        }

        fn list_roles(
            &self,
        ) -> Result<Box<dyn ExactSizeIterator<Item = Role>>, RoleBasedAuthorizationStoreError>
        {
            Ok(Box::new(
                self.roles
                    .lock()
                    .expect("mem role based authorization store lock was poisoned")
                    .values()
                    .cloned()
                    .collect::<Vec<_>>()
                    .into_iter(),
            ))
        }

        fn add_role(&self, role: Role) -> Result<(), RoleBasedAuthorizationStoreError> {
            let mut roles = self
                .roles
                .lock()
                .expect("mem role based authorization store lock was poisoned");

            if !roles.contains_key(role.id()) {
                roles.insert(role.id().to_string(), role);
                Ok(())
            } else {
                Err(RoleBasedAuthorizationStoreError::ConstraintViolation(
                    ConstraintViolationError::with_violation_type(ConstraintViolationType::Unique),
                ))
            }
        }

        fn update_role(&self, role: Role) -> Result<(), RoleBasedAuthorizationStoreError> {
            self.roles
                .lock()
                .expect("mem role based authorization store lock was poisoned")
                .insert(role.id().to_string(), role);

            Ok(())
        }

        fn remove_role(&self, role_id: &str) -> Result<(), RoleBasedAuthorizationStoreError> {
            self.roles
                .lock()
                .expect("mem role based authorization store lock was poisoned")
                .remove(role_id);

            Ok(())
        }

        fn get_assignment(
            &self,
            _identity: &Identity,
        ) -> Result<Option<Assignment>, RoleBasedAuthorizationStoreError> {
            unimplemented!()
        }

        fn get_assigned_roles(
            &self,
            _identity: &Identity,
        ) -> Result<Box<dyn ExactSizeIterator<Item = Role>>, RoleBasedAuthorizationStoreError>
        {
            unimplemented!()
        }

        fn list_assignments(
            &self,
        ) -> Result<Box<dyn ExactSizeIterator<Item = Assignment>>, RoleBasedAuthorizationStoreError>
        {
            unimplemented!()
        }

        fn add_assignment(
            &self,
            _assignment: Assignment,
        ) -> Result<(), RoleBasedAuthorizationStoreError> {
            unimplemented!()
        }

        fn update_assignment(
            &self,
            _assignment: Assignment,
        ) -> Result<(), RoleBasedAuthorizationStoreError> {
            unimplemented!()
        }

        fn remove_assignment(
            &self,
            _identity: &Identity,
        ) -> Result<(), RoleBasedAuthorizationStoreError> {
            unimplemented!()
        }

        fn clone_box(&self) -> Box<dyn RoleBasedAuthorizationStore> {
            Box::new(self.clone())
        }
    }
}
