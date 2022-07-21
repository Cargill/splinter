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

//! Rest API Module.
//!
//! Module for creating REST APIs for services.
//!
//! Below is an example of a `struct` that implements `ResourceProvider` and then passes its resources
//! to a running instance of `RestApi`.
//!
//! ```
//! use actix_web::HttpResponse;
//! use cylinder::{VerifierFactory, secp256k1::Secp256k1Context};
//! use futures::IntoFuture;
//! use splinter::rest_api::{
//!     AuthConfig, Resource, Method, RestApiBuilder, RestResourceProvider,
//!     auth::authorization::Permission,
//! };
//!
//! struct IndexResource {
//!     pub name: String
//! }
//!
//! impl RestResourceProvider for IndexResource {
//!     fn resources(&self) -> Vec<Resource> {
//!         let name = self.name.clone();
//!
//!         vec![Resource::build("/index").add_method(
//!             Method::Get,
//!             Permission::AllowUnauthenticated,
//!             move |r, p| {
//!                 Box::new(
//!                     HttpResponse::Ok()
//!                     .body(format!("Hello, I am {}", name))
//!                     .into_future())
//!             },
//!         )]
//!     }
//! }
//!
//! let index_resource = IndexResource { name: "Taco".to_string() };
//!
//! #[cfg(not(feature = "https-bind"))]
//! let bind = "localhost:8080";
//! #[cfg(feature = "https-bind")]
//! let bind = splinter::rest_api::BindConfig::Http("localhost:8080".into());
//!
//! RestApiBuilder::new()
//!     .add_resources(index_resource.resources())
//!     .with_bind(bind)
//!     .with_auth_configs(vec![AuthConfig::Cylinder{
//!         verifier: Secp256k1Context::new().new_verifier(),
//!     }])
//!     .build()
//!     .unwrap()
//!     .run();
//! ```

#[cfg(feature = "rest-api-actix-web-1")]
pub mod actix_web_1;
pub mod auth;
mod bind_config;
#[cfg(feature = "rest-api-cors")]
pub mod cors;
mod errors;
#[cfg(feature = "oauth")]
mod oauth_config;
pub mod paging;
mod response_models;
pub mod secrets;
pub mod sessions;

use percent_encoding::{AsciiSet, CONTROLS};

#[cfg(all(feature = "oauth", feature = "rest-api-actix-web-1"))]
use crate::oauth::rest_api::OAuthResourceProvider;

pub use bind_config::BindConfig;
pub use errors::{RequestError, RestApiServerError};
#[cfg(feature = "oauth")]
pub use oauth_config::OAuthConfig;

pub use response_models::ErrorResponse;

#[cfg(feature = "rest-api-actix-web-1")]
pub use actix_web_1::{
    get_authorization_token, into_bytes, into_protobuf, new_websocket_event_sender, require_header,
    AuthConfig, Continuation, EventSender, HandlerFunction, Method, ProtocolVersionRangeGuard,
    Request, RequestGuard, Resource, Response, ResponseError, RestApi, RestApiBuilder,
    RestApiShutdownHandle, RestResourceProvider,
};

#[cfg(any(
    feature = "admin-service-event-client-actix-web-client",
    feature = "authorization",
    feature = "biome-credentials",
    feature = "biome-key-management",
    all(feature = "oauth", feature = "rest-api-actix-web-1"),
))]
pub(crate) const SPLINTER_PROTOCOL_VERSION: u32 = 2;

const QUERY_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'<')
    .add(b'>')
    .add(b'`')
    .add(b'=')
    .add(b'!')
    .add(b'{')
    .add(b'}')
    .add(b'[')
    .add(b']')
    .add(b':')
    .add(b',');

pub fn percent_encode_filter_query(input: &str) -> String {
    percent_encoding::utf8_percent_encode(input, QUERY_ENCODE_SET).to_string()
}
