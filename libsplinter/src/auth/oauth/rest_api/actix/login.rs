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

//! The `GET /oauth/login` endpoint for getting the authorization request URL for the provider.

use actix_web::{http::header::LOCATION, HttpResponse};
use futures::future::IntoFuture;

use crate::auth::oauth::OAuthClient;
use crate::protocol;
use crate::rest_api::{ErrorResponse, Method, ProtocolVersionRangeGuard, Resource};

pub fn make_login_route(client: OAuthClient) -> Resource {
    Resource::build("/oauth/login")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::OAUTH_LOGIN_MIN,
            protocol::OAUTH_PROTOCOL_VERSION,
        ))
        .add_method(Method::Get, move |_, _| {
            Box::new(
                match client.get_authorization_url() {
                    Ok(auth_url) => HttpResponse::Found().header(LOCATION, auth_url).finish(),
                    Err(err) => {
                        error!("{}", err);
                        HttpResponse::InternalServerError().json(ErrorResponse::internal_error())
                    }
                }
                .into_future(),
            )
        })
}
