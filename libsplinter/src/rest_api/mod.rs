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

//! Rest API Module.
//!
//! Module for creating REST APIs for services.
//!
//! Below is an example of a `struct` that implements `ResouceProvider` and then passes its resources
//! to a running instance of `RestApi`.
//!
//! ```ignore
//! use splinter::rest_api::{Resource, Method, RestApiBuilder, RestResourceProvider};
//! use actix_web::HttpResponse;
//! use futures::IntoFuture;
//!
//! struct IndexResource {
//!     pub name: String
//! }
//!
//! impl RestResourceProvider for IndexResource {
//!     fn resources(&self) -> Vec<Resource> {
//!         let name = self.name.clone();
//!
//!         vec![Resource::build("/index").add_method(Method::Get, move |r, p| {
//!             Box::new(
//!                 HttpResponse::Ok()
//!                 .body(format!("Hello, I am {}", name))
//!                 .into_future())
//!         })]
//!     }
//! }
//!
//! let index_resource = IndexResource { name: "Taco".to_string() };
//!
//! #[cfg(not(feature = "https-bind"))]
//! let bind = "localhost:8080";
//! #[cfg(feature = "https-bind")]
//! let bind = splinter::rest_api::RestApiBind::Insecure("localhost:8080".into());
//!
//! RestApiBuilder::new()
//!     .add_resources(index_resource.resources())
//!     .with_bind(bind)
//!     .build()
//!     .unwrap()
//!     .run();
//! ```

pub(crate) mod actix_web_1;
#[cfg(feature = "auth")]
pub mod auth;
#[cfg(feature = "rest-api-cors")]
pub mod cors;
mod errors;
pub mod paging;
mod response_models;
pub mod secrets;
pub mod sessions;

use percent_encoding::{AsciiSet, CONTROLS};

#[cfg(feature = "oauth")]
use std::boxed::Box;

#[cfg(feature = "oauth")]
use crate::oauth::rest_api::OAuthResourceProvider;
#[cfg(any(feature = "oauth-github", feature = "oauth-openid"))]
use crate::oauth::store::InflightOAuthRequestStore;

pub use errors::{RequestError, RestApiServerError};

pub use response_models::ErrorResponse;

#[cfg(feature = "auth")]
pub use actix_web_1::AuthConfig;
pub use actix_web_1::{
    get_authorization_token, get_paging_query, into_bytes, into_protobuf,
    new_websocket_event_sender, require_header, BadPagingRequest, Continuation, EventSender,
    HandlerFunction, Method, ProtocolVersionRangeGuard, Request, RequestGuard, Resource, Response,
    ResponseError, RestApi, RestApiBuilder, RestApiShutdownHandle, RestResourceProvider,
};

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

/// Bind configuration for the REST API.
#[derive(Clone)]
pub enum RestApiBind {
    #[cfg(feature = "https-bind")]
    /// A secure binding, including certificate and key paths.
    Secure {
        bind: String,
        cert_path: String,
        key_path: String,
    },
    /// A insecure binding.
    Insecure(String),
}

impl std::fmt::Display for RestApiBind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            #[cfg(feature = "https-bind")]
            RestApiBind::Secure { bind, .. } => write!(f, "https://{}", bind),
            RestApiBind::Insecure(bind) => write!(f, "http://{}", bind),
        }
    }
}

/// OAuth configurations that are supported out-of-the-box by the Splinter REST API.
#[cfg(feature = "oauth")]
pub enum OAuthConfig {
    #[cfg(feature = "oauth-openid")]
    Azure {
        /// The client ID of the Azure OAuth app
        client_id: String,
        /// The client secret of the Azure OAuth app
        client_secret: String,
        /// The redirect URL that is configured for the Azure OAuth app
        redirect_url: String,
        /// The URL of the OpenID discovery document for the Azure OAuth app
        oauth_openid_url: String,
        /// The store for in-flight requests
        inflight_request_store: Box<dyn InflightOAuthRequestStore>,
    },
    /// OAuth provided by GitHub
    #[cfg(feature = "oauth-github")]
    GitHub {
        /// The client ID of the GitHub OAuth app
        client_id: String,
        /// The client secret of the GitHub OAuth app
        client_secret: String,
        /// The redirect URL that is configured for the GitHub OAuth app
        redirect_url: String,
        /// The store for in-flight requests
        inflight_request_store: Box<dyn InflightOAuthRequestStore>,
    },
    #[cfg(feature = "oauth-openid")]
    Google {
        /// The client ID of the Google OAuth app
        client_id: String,
        /// The client secret of the Google OAuth app
        client_secret: String,
        /// The redirect URL that is configured for the Google OAuth app
        redirect_url: String,
        /// The store for in-flight requests
        inflight_request_store: Box<dyn InflightOAuthRequestStore>,
    },
    #[cfg(feature = "oauth-openid")]
    OpenId {
        /// The client ID of the OpenId OAuth app
        client_id: String,
        /// The client secret of the OpenId OAuth app
        client_secret: String,
        /// The redirect URL that is configured for the OpenId OAuth app
        redirect_url: String,
        /// The URL of the OpenID discovery document for the OpenId OAuth app
        oauth_openid_url: String,
        /// The store for in-flight requests
        inflight_request_store: Box<dyn InflightOAuthRequestStore>,
    },
}

pub fn percent_encode_filter_query(input: &str) -> String {
    percent_encoding::utf8_percent_encode(input, QUERY_ENCODE_SET).to_string()
}
