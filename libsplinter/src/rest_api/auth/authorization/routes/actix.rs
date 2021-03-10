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
//! * `GET /authroization/permissions` for displaying all REST API permissions

use actix_web::HttpResponse;
use futures::future::IntoFuture;

use crate::protocol;
use crate::rest_api::{
    actix_web_1::{Method, ProtocolVersionRangeGuard, Resource},
    auth::authorization::Permission,
};

use super::{resources::PermissionResponse, AUTHORIZATION_PERMISSIONS_READ_PERMISSION};

pub fn make_permissions_resource(permissions: Vec<Permission>) -> Resource {
    let permissions = permissions
        .into_iter()
        // Add this endpoint's own permission
        .chain(std::iter::once(AUTHORIZATION_PERMISSIONS_READ_PERMISSION))
        // Deduplicate and convert to serializable response structs
        .fold(vec![], |mut perms: Vec<PermissionResponse>, perm| {
            // Only interested in assignable permissions
            if let Permission::Check {
                permission_id,
                permission_display_name,
                permission_description,
            } = perm
            {
                if !perms
                    .iter()
                    .any(|existing_perm| permission_id == existing_perm.permission_id)
                {
                    perms.push(PermissionResponse {
                        permission_id,
                        permission_display_name,
                        permission_description,
                    });
                }
            }
            perms
        });

    Resource::build("/authorization/permissions")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::AUTHORIZATION_PERMISSIONS_MIN,
            protocol::AUTHORIZATION_PROTOCOL_VERSION,
        ))
        .add_method(
            Method::Get,
            AUTHORIZATION_PERMISSIONS_READ_PERMISSION,
            move |_, _| {
                Box::new(
                    HttpResponse::Ok()
                        .json(json!({
                            "data": &permissions,
                        }))
                        .into_future(),
                )
            },
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    use reqwest::{blocking::Client, StatusCode, Url};

    use crate::rest_api::actix_web_1::{RestApiBuilder, RestApiShutdownHandle};

    const PERM1: Permission = Permission::Check {
        permission_id: "id1",
        permission_display_name: "display name 1",
        permission_description: "description 1",
    };
    const PERM2: Permission = Permission::Check {
        permission_id: "id2",
        permission_display_name: "display name 2",
        permission_description: "description 2",
    };

    /// Verifies that the `GET /authorization/permissions` endpoint returns the correct values
    ///
    /// 1. Start the REST API with a duplicated permission, a non-duplicated permission, an
    ///    "allow authenticated" permission, and an "allow unauthenticated" permission
    /// 2. Make a request to the endpoint and verify the resulting permissions list contains only
    ///    the non-duplciated permision, a single instance of the duplicated permission, and the
    ///    endpoint's own permission
    /// 3. Shutdown the REST API
    #[test]
    fn get_permissions() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_permissions_resource(vec![
                PERM1,
                PERM1,
                PERM2,
                Permission::AllowAuthenticated,
                Permission::AllowUnauthenticated,
            ])]);

        let url = Url::parse(&format!("http://{}/authorization/permissions", bind_url))
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

        let permissions = resp
            .json::<Response>()
            .expect("Failed to parse response body")
            .data;
        assert_eq!(permissions.len(), 3);
        match PERM1 {
            Permission::Check {
                permission_id,
                permission_display_name,
                permission_description,
            } => {
                assert!(permissions
                    .iter()
                    .any(|perm| perm.permission_id == permission_id
                        && perm.permission_display_name == permission_display_name
                        && perm.permission_description == permission_description));
            }
            _ => unreachable!(),
        }
        match PERM2 {
            Permission::Check {
                permission_id,
                permission_display_name,
                permission_description,
            } => {
                assert!(permissions
                    .iter()
                    .any(|perm| perm.permission_id == permission_id
                        && perm.permission_display_name == permission_display_name
                        && perm.permission_description == permission_description));
            }
            _ => unreachable!(),
        }
        match AUTHORIZATION_PERMISSIONS_READ_PERMISSION {
            Permission::Check {
                permission_id,
                permission_display_name,
                permission_description,
            } => {
                assert!(permissions
                    .iter()
                    .any(|perm| perm.permission_id == permission_id
                        && perm.permission_display_name == permission_display_name
                        && perm.permission_description == permission_description));
            }
            _ => unreachable!(),
        }

        shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[derive(Deserialize)]
    struct Response {
        data: Vec<PermissionData>,
    }

    #[derive(Deserialize)]
    struct PermissionData {
        permission_id: String,
        permission_display_name: String,
        permission_description: String,
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
}
