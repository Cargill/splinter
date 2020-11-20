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
//! ```no_run
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
//! RestApiBuilder::new()
//!     .add_resources(index_resource.resources())
//!     .with_bind("localhost:8080")
//!     .build()
//!     .unwrap()
//!     .run();
//! ```

#[cfg(feature = "rest-api-cors")]
pub mod cors;
mod errors;
mod events;
pub mod paging;
mod response_models;
pub mod secrets;
pub mod sessions;

use actix_web::{
    error::ErrorBadRequest, http::header, middleware, web, App, Error as ActixError, HttpRequest,
    HttpResponse, HttpServer,
};
#[cfg(all(feature = "auth", feature = "cylinder-jwt"))]
use cylinder::Verifier;
use futures::{future::FutureResult, stream::Stream, Future, IntoFuture};
use percent_encoding::{AsciiSet, CONTROLS};
use protobuf::{self, Message};
use std::boxed::Box;
#[cfg(all(feature = "auth", feature = "cylinder-jwt"))]
use std::sync::Mutex;
use std::sync::{mpsc, Arc};
use std::thread;

#[cfg(feature = "oauth")]
use crate::auth::oauth::{
    rest_api::{OAuthResourceProvider, SaveUserInfoOperation, SaveUserInfoToNull},
    OAuthClient,
};
#[cfg(all(feature = "auth", feature = "cylinder-jwt"))]
use crate::auth::rest_api::identity::cylinder::CylinderKeyIdentityProvider;
#[cfg(feature = "oauth-github")]
use crate::auth::rest_api::identity::github::GithubUserIdentityProvider;
#[cfg(feature = "auth")]
use crate::auth::rest_api::{
    actix::Authorization,
    identity::{AuthorizationMapping, IdentityProvider},
};
#[cfg(all(feature = "auth", feature = "biome-credentials"))]
use crate::biome::rest_api::BiomeRestResourceManager;
#[cfg(feature = "biome-oauth")]
use crate::biome::{
    oauth::store::OAuthProvider,
    rest_api::auth::{GetUserByOAuthAuthorization, OAuthUserStoreSaveUserInfoOperation},
    OAuthUserStore, UserStore,
};
#[cfg(feature = "auth")]
use crate::error::InvalidStateError;

pub use errors::{RequestError, ResponseError, RestApiServerError};

pub use events::{new_websocket_event_sender, EventSender};

pub use response_models::ErrorResponse;

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

/// A `RestResourceProvider` provides a list of resources that are consumed by `RestApi`.
pub trait RestResourceProvider {
    fn resources(&self) -> Vec<Resource>;
}

pub type HandlerFunction = Box<
    dyn Fn(HttpRequest, web::Payload) -> Box<dyn Future<Item = HttpResponse, Error = ActixError>>
        + Send
        + Sync
        + 'static,
>;

/// Shutdown handle returned by `RestApi::run`. Allows rest api instance to be shut down
/// gracefully.
pub struct RestApiShutdownHandle {
    do_shutdown: Box<dyn Fn() -> Result<(), RestApiServerError> + Send>,
    port_numbers: Vec<u16>,
}

impl RestApiShutdownHandle {
    pub fn shutdown(&self) -> Result<(), RestApiServerError> {
        (*self.do_shutdown)()
    }

    pub fn port_numbers(&self) -> Vec<u16> {
        self.port_numbers.clone()
    }
}

pub struct Request(HttpRequest, web::Payload);

impl From<(HttpRequest, web::Payload)> for Request {
    fn from((http_request, payload): (HttpRequest, web::Payload)) -> Self {
        Self(http_request, payload)
    }
}

impl Into<(HttpRequest, web::Payload)> for Request {
    fn into(self) -> (HttpRequest, web::Payload) {
        (self.0, self.1)
    }
}

pub struct Response(HttpResponse);

impl From<HttpResponse> for Response {
    fn from(res: HttpResponse) -> Self {
        Self(res)
    }
}

impl IntoFuture for Response {
    type Item = HttpResponse;
    type Error = ActixError;
    type Future = FutureResult<HttpResponse, ActixError>;

    fn into_future(self) -> Self::Future {
        self.0.into_future()
    }
}

impl std::fmt::Debug for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

/// Rest methods compatible with `RestApi`.
#[derive(Clone)]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Method::Get => f.write_str("GET"),
            Method::Post => f.write_str("POST"),
            Method::Put => f.write_str("PUT"),
            Method::Patch => f.write_str("PATCH"),
            Method::Delete => f.write_str("DELETE"),
            Method::Head => f.write_str("HEAD"),
        }
    }
}

/// `Resource` represents a RESTful endpoint.
///
/// ```
/// use splinter::rest_api::{Resource, Method};
/// use actix_web::HttpResponse;
/// use futures::IntoFuture;
///
/// Resource::build("/index")
///     .add_method(Method::Get, |r, p| {
///         Box::new(
///             HttpResponse::Ok()
///                 .body("Hello, World")
///                 .into_future()
///         )
///     });
/// ```
#[derive(Clone)]
pub struct Resource {
    route: String,
    request_guards: Vec<Arc<dyn RequestGuard>>,
    methods: Vec<(Method, Arc<HandlerFunction>)>,
}

impl Resource {
    #[deprecated(note = "Please use the `build` and `add_method` methods instead")]
    pub fn new<F>(method: Method, route: &str, handle: F) -> Self
    where
        F: Fn(
                HttpRequest,
                web::Payload,
            ) -> Box<dyn Future<Item = HttpResponse, Error = ActixError>>
            + Send
            + Sync
            + 'static,
    {
        Self::build(route).add_method(method, handle)
    }

    pub fn build(route: &str) -> Self {
        Self {
            route: route.to_string(),
            methods: vec![],
            request_guards: vec![],
        }
    }

    pub fn add_method<F>(mut self, method: Method, handle: F) -> Self
    where
        F: Fn(
                HttpRequest,
                web::Payload,
            ) -> Box<dyn Future<Item = HttpResponse, Error = ActixError>>
            + Send
            + Sync
            + 'static,
    {
        self.methods.push((method, Arc::new(Box::new(handle))));
        self
    }

    /// Adds a RequestGuard to the given Resource.
    ///
    /// This guard applies to all methods defined for this resource.
    ///
    /// # Example
    ///
    /// ```
    /// use splinter::rest_api::{Resource, Method, Continuation};
    /// use actix_web::{HttpRequest, HttpResponse};
    /// use futures::IntoFuture;
    ///
    /// Resource::build("/index")
    ///     .add_request_guard(|r: &HttpRequest| {
    ///         if !r.headers().contains_key("GuardFlag") {
    ///             Continuation::terminate(
    ///                 HttpResponse::BadRequest().finish().into_future(),
    ///             )
    ///         } else {
    ///             Continuation::Continue
    ///         }
    ///     })
    ///     .add_method(Method::Get, |r, p| {
    ///         Box::new(
    ///             HttpResponse::Ok()
    ///                 .body("Hello, World")
    ///                 .into_future()
    ///         )
    ///     });
    /// ```
    pub fn add_request_guard<RG>(mut self, guard: RG) -> Self
    where
        RG: RequestGuard + Clone + 'static,
    {
        self.request_guards.push(Arc::new(guard));
        self
    }

    fn into_route(self) -> actix_web::Resource {
        let mut resource = web::resource(&self.route);

        let mut allowed_methods = self
            .methods
            .iter()
            .map(|(method, _)| method.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        allowed_methods += ", OPTIONS";

        resource = resource.route(web::route().guard(actix_web::guard::Options()).to(
            move |_: HttpRequest| {
                HttpResponse::Ok()
                    .header(header::ALLOW, allowed_methods.clone())
                    .finish()
            },
        ));

        let request_guards = self.request_guards;
        self.methods
            .into_iter()
            .fold(resource, |resource, (method, handler)| {
                let guards = request_guards.clone();
                let func = move |r: HttpRequest, p: web::Payload| {
                    // This clone satisfies a requirement that this be FnOnce
                    if !guards.is_empty() {
                        for guard in guards.clone() {
                            match guard.evaluate(&r) {
                                Continuation::Terminate(result) => return result,
                                Continuation::Continue => (),
                            }
                        }
                    }
                    (handler)(r, p)
                };
                resource.route(match method {
                    Method::Get => web::get().to_async(func),
                    Method::Post => web::post().to_async(func),
                    Method::Put => web::put().to_async(func),
                    Method::Patch => web::patch().to_async(func),
                    Method::Delete => web::delete().to_async(func),
                    Method::Head => web::head().to_async(func),
                })
            })
    }
}

/// A continuation indicates whether or not a guard should allow a given request to continue, or to
/// return a result.
pub enum Continuation {
    Continue,
    Terminate(Box<dyn Future<Item = HttpResponse, Error = ActixError>>),
}

impl Continuation {
    /// Wraps the given future in the Continuation::Terminate variant.
    pub fn terminate<F>(fut: F) -> Continuation
    where
        F: Future<Item = HttpResponse, Error = ActixError> + 'static,
    {
        Continuation::Terminate(Box::new(fut))
    }
}

/// A guard checks the request content in advance, and either continues the request, or
/// returns a terminating result.
pub trait RequestGuard: Send + Sync {
    /// Evaluates the request and determines whether or not the request should be continued or
    /// short-circuited with a terminating future.
    fn evaluate(&self, req: &HttpRequest) -> Continuation;
}

impl<F> RequestGuard for F
where
    F: Fn(&HttpRequest) -> Continuation + Sync + Send,
{
    fn evaluate(&self, req: &HttpRequest) -> Continuation {
        (*self)(req)
    }
}

impl RequestGuard for Box<dyn RequestGuard> {
    fn evaluate(&self, req: &HttpRequest) -> Continuation {
        (**self).evaluate(req)
    }
}

/// Guards requests based on a minimum protocol version.
///
/// A protocol version is specified via the HTTP header `"SplinterProtocolVersion"`.  This header
/// is a positive integer value.
#[derive(Clone)]
pub struct ProtocolVersionRangeGuard {
    min: u32,
    max: u32,
}

impl ProtocolVersionRangeGuard {
    /// Constructs a new protocol version guard with the given minimum.
    pub fn new(min: u32, max: u32) -> Self {
        Self { min, max }
    }
}

impl RequestGuard for ProtocolVersionRangeGuard {
    fn evaluate(&self, req: &HttpRequest) -> Continuation {
        if let Some(header_value) = req.headers().get("SplinterProtocolVersion") {
            let parsed_header = header_value
                .to_str()
                .map_err(|err| {
                    format!(
                        "Invalid characters in SplinterProtocolVersion header: {}",
                        err
                    )
                })
                .and_then(|val_str| {
                    val_str.parse::<u32>().map_err(|_| {
                        "SplinterProtocolVersion must be a valid positive integer".to_string()
                    })
                });
            match parsed_header {
                Err(msg) => Continuation::terminate(
                    HttpResponse::BadRequest()
                        .json(json!({
                            "message": msg,
                        }))
                        .into_future(),
                ),
                Ok(version) if version < self.min => Continuation::terminate(
                    HttpResponse::BadRequest()
                        .json(json!({
                            "message": format!(
                                "Client must support protocol version {} or greater.",
                                self.min,
                            ),
                            "requested_protocol": version,
                            "splinter_protocol": self.max,
                            "libsplinter_version": format!(
                                "{}.{}.{}",
                                env!("CARGO_PKG_VERSION_MAJOR"),
                                env!("CARGO_PKG_VERSION_MINOR"),
                                env!("CARGO_PKG_VERSION_PATCH")
                            )
                        }))
                        .into_future(),
                ),
                Ok(version) if version > self.max => Continuation::terminate(
                    HttpResponse::BadRequest()
                        .json(json!({
                            "message": format!(
                                "Client requires a newer protocol than can be provided: {} > {}",
                                version,
                                self.max,
                            ),
                            "requested_protocol": version,
                            "splinter_protocol": self.max,
                            "libsplinter_version": format!(
                                "{}.{}.{}",
                                env!("CARGO_PKG_VERSION_MAJOR"),
                                env!("CARGO_PKG_VERSION_MINOR"),
                                env!("CARGO_PKG_VERSION_PATCH")
                            )
                        }))
                        .into_future(),
                ),
                Ok(_) => Continuation::Continue,
            }
        } else {
            // Ignore the missing header, and assume the client will handle version mismatches by
            // inspecting the output
            Continuation::Continue
        }
    }
}

#[cfg(feature = "auth")]
struct ConfigureAuthorizationMapping {
    config_fn: Box<dyn FnMut(Authorization) -> Authorization>,
}

#[cfg(feature = "auth")]
impl ConfigureAuthorizationMapping {
    fn new<M, T>(auth_mapping: M) -> Self
    where
        T: 'static,
        M: AuthorizationMapping<T> + Send + Sync + 'static,
    {
        let mut auth_mapping = Some(auth_mapping);
        Self {
            config_fn: Box::new(move |authorization| {
                if let Some(auth_mapping) = auth_mapping.take() {
                    authorization.with_authorization_mapping(auth_mapping)
                } else {
                    authorization
                }
            }),
        }
    }

    fn configure(&mut self, authorization: Authorization) -> Authorization {
        (*self.config_fn)(authorization)
    }
}

/// `RestApi` is used to create an instance of a restful web server.
pub struct RestApi {
    resources: Vec<Resource>,
    bind: String,
    #[cfg(feature = "rest-api-cors")]
    whitelist: Option<Vec<String>>,
    #[cfg(feature = "auth")]
    identity_providers: Vec<Box<dyn IdentityProvider>>,
    #[cfg(feature = "auth")]
    authorization_mappings: Vec<ConfigureAuthorizationMapping>,
}

impl RestApi {
    pub fn run(
        self,
    ) -> Result<(RestApiShutdownHandle, thread::JoinHandle<()>), RestApiServerError> {
        let (tx, rx) = mpsc::channel();

        let bind_url_for_err = self.bind.to_owned();
        let bind_url = self.bind.to_owned();
        let resources = self.resources;
        #[cfg(feature = "rest-api-cors")]
        let whitelist = self.whitelist;
        #[cfg(feature = "auth")]
        let mut authorization = Authorization::new(self.identity_providers.to_owned());

        #[cfg(feature = "auth")]
        {
            let mut auth_mappings = self.authorization_mappings;
            for auth_mapping in auth_mappings.iter_mut() {
                authorization = auth_mapping.configure(authorization);
            }
        }

        #[cfg(feature = "rest-api-cors")]
        let cors = match &whitelist {
            Some(list) => cors::Cors::new(list.to_vec()),
            None => cors::Cors::new_allow_any(),
        };

        let join_handle = thread::Builder::new()
            .name("SplinterDRestApi".into())
            .spawn(move || {
                let sys = actix::System::new("SplinterD-Rest-API");
                let mut server = HttpServer::new(move || {
                    let app = App::new();

                    #[cfg(feature = "rest-api-cors")]
                    let app = app.wrap(cors.clone());

                    #[cfg(feature = "auth")]
                    let app = app.wrap(authorization.clone());

                    let mut app = app.wrap(middleware::Logger::default());

                    for resource in resources.clone() {
                        app = app.service(resource.into_route());
                    }
                    app
                });

                server = match server.bind(&bind_url) {
                    Ok(server) => server,
                    Err(err) => {
                        let error_msg = format!("Invalid REST API bind {}: {}", bind_url, err);
                        error!("{}", error_msg);
                        if let Err(err) = tx.send(Err(error_msg)) {
                            error!("Failed to notify receiver of bind error: {}", err);
                        }
                        return;
                    }
                };
                let port_numbers = server.addrs().iter().map(|addrs| addrs.port()).collect();

                let addr = server.disable_signals().system_exit().start();

                if let Err(err) = tx.send(Ok((addr, port_numbers))) {
                    error!("Unable to send Server Addr: {}", err);
                }

                if let Err(err) = sys.run() {
                    error!("REST Api unexpectedly exiting: {}", err);
                };

                info!("Rest API terminating");
            })?;

        let (addr, port_numbers) = rx
            .recv()
            .map_err(|err| {
                RestApiServerError::StartUpError(format!("Unable to receive Server Addr: {}", err))
            })?
            .map_err(|err| {
                RestApiServerError::BindError(format!(
                    "Failed to bind to URL {}: {}",
                    bind_url_for_err, err
                ))
            })?;

        let do_shutdown = Box::new(move || {
            debug!("Shutting down Rest API");
            if let Err(err) = addr.stop(true).wait() {
                error!("An error occured while shutting down rest API: {:?}", err);
            }
            debug!("Graceful signal sent to Rest API");

            Ok(())
        });

        Ok((
            RestApiShutdownHandle {
                do_shutdown,
                port_numbers,
            },
            join_handle,
        ))
    }

    /// Builds the `RestApi` without requiring any security configuration
    #[cfg(test)]
    pub fn run_insecure(
        self,
    ) -> Result<(RestApiShutdownHandle, thread::JoinHandle<()>), RestApiServerError> {
        let (tx, rx) = mpsc::channel();

        let bind_url = self.bind.to_owned();
        let resources = self.resources.to_owned();
        #[cfg(feature = "rest-api-cors")]
        let whitelist = self.whitelist.to_owned();

        #[cfg(feature = "rest-api-cors")]
        let cors = match &whitelist {
            Some(list) => cors::Cors::new(list.to_vec()),
            None => cors::Cors::new_allow_any(),
        };

        let join_handle = thread::Builder::new()
            .name("SplinterDRestApi".into())
            .spawn(move || {
                let sys = actix::System::new("SplinterD-Rest-API");
                let mut server = HttpServer::new(move || {
                    let app = App::new();

                    #[cfg(feature = "rest-api-cors")]
                    let app = app.wrap(cors.clone());

                    let mut app = app.wrap(middleware::Logger::default());

                    for resource in resources.clone() {
                        app = app.service(resource.into_route());
                    }
                    app
                });

                server = match server.bind(&bind_url) {
                    Ok(server) => server,
                    Err(err) => {
                        let error_msg = format!("Invalid REST API bind {}: {}", bind_url, err);
                        error!("{}", error_msg);
                        if let Err(err) = tx.send(Err(error_msg)) {
                            error!("Failed to notify receiver of bind error: {}", err);
                        }
                        return;
                    }
                };
                let port_numbers = server.addrs().iter().map(|addrs| addrs.port()).collect();

                let addr = server.disable_signals().system_exit().start();

                if let Err(err) = tx.send(Ok((addr, port_numbers))) {
                    error!("Unable to send Server Addr: {}", err);
                }

                if let Err(err) = sys.run() {
                    error!("REST Api unexpectedly exiting: {}", err);
                };

                info!("Rest API terminating");
            })?;

        let (addr, port_numbers) = rx
            .recv()
            .map_err(|err| {
                RestApiServerError::StartUpError(format!("Unable to receive Server Addr: {}", err))
            })?
            .map_err(|err| {
                RestApiServerError::BindError(format!(
                    "Failed to bind to URL {}: {}",
                    self.bind, err
                ))
            })?;

        let do_shutdown = Box::new(move || {
            debug!("Shutting down Rest API");
            if let Err(err) = addr.stop(true).wait() {
                error!("An error occured while shutting down rest API: {:?}", err);
            }
            debug!("Graceful signal sent to Rest API");

            Ok(())
        });

        Ok((
            RestApiShutdownHandle {
                do_shutdown,
                port_numbers,
            },
            join_handle,
        ))
    }
}

/// Builder `struct` for `RestApi`.
pub struct RestApiBuilder {
    resources: Vec<Resource>,
    bind: Option<String>,
    #[cfg(feature = "rest-api-cors")]
    whitelist: Option<Vec<String>>,
    #[cfg(feature = "auth")]
    auth_configs: Vec<AuthConfig>,
    #[cfg(feature = "auth")]
    authorization_mappings: Vec<ConfigureAuthorizationMapping>,
}

impl Default for RestApiBuilder {
    fn default() -> Self {
        Self {
            resources: Vec::new(),
            bind: None,
            #[cfg(feature = "rest-api-cors")]
            whitelist: None,
            #[cfg(feature = "auth")]
            auth_configs: Vec::new(),
            #[cfg(feature = "auth")]
            authorization_mappings: vec![],
        }
    }
}

impl RestApiBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_bind(mut self, value: &str) -> Self {
        self.bind = Some(value.to_string());
        self
    }

    pub fn add_resource(mut self, value: Resource) -> Self {
        self.resources.push(value);
        self
    }

    pub fn add_resources(mut self, mut values: Vec<Resource>) -> Self {
        self.resources.append(&mut values);
        self
    }

    #[cfg(feature = "rest-api-cors")]
    pub fn with_whitelist(mut self, values: Vec<String>) -> Self {
        self.whitelist = Some(values);
        self
    }

    #[cfg(feature = "auth")]
    pub fn with_auth_configs(mut self, auth_configs: Vec<AuthConfig>) -> Self {
        self.auth_configs = auth_configs;
        self
    }

    #[cfg(feature = "auth")]
    pub fn with_authorization_mapping<M, T>(mut self, authorization_mapping: M) -> Self
    where
        T: 'static,
        M: AuthorizationMapping<T> + Send + Sync + 'static,
    {
        self.authorization_mappings
            .push(ConfigureAuthorizationMapping::new(authorization_mapping));

        self
    }

    // Allowing unused_mut because self must be mutable if feature `auth` is enabled
    #[allow(unused_mut)]
    pub fn build(mut self) -> Result<RestApi, RestApiServerError> {
        let bind = self
            .bind
            .ok_or_else(|| RestApiServerError::MissingField("bind".to_string()))?;

        #[cfg(feature = "auth")]
        let identity_providers = {
            if self.auth_configs.is_empty() {
                return Err(RestApiServerError::InvalidStateError(
                    InvalidStateError::with_message(
                        "REST API auth is enabled, but no auth has been configured".to_string(),
                    ),
                ));
            }

            let mut identity_providers = Vec::<Box<dyn IdentityProvider>>::new();
            #[cfg(feature = "oauth")]
            let mut oauth_configured = false;

            for auth_config in self.auth_configs {
                match auth_config {
                    #[cfg(feature = "biome-credentials")]
                    AuthConfig::Biome {
                        biome_resource_manager,
                    } => {
                        identity_providers
                            .push(Box::new(biome_resource_manager.get_identity_provider()));
                        self.authorization_mappings
                            .push(ConfigureAuthorizationMapping::new(
                                biome_resource_manager.get_authorization_mapping(),
                            ));
                        self.resources
                            .append(&mut biome_resource_manager.resources());
                    }
                    #[cfg(feature = "cylinder-jwt")]
                    AuthConfig::Cylinder { verifier } => {
                        identity_providers.push(Box::new(CylinderKeyIdentityProvider::new(
                            Arc::new(Mutex::new(verifier)),
                        )));
                    }
                    #[cfg(feature = "oauth")]
                    AuthConfig::OAuth {
                        oauth_config,
                        user_info_save_config,
                    } => {
                        if oauth_configured {
                            return Err(RestApiServerError::InvalidStateError(
                                InvalidStateError::with_message(
                                    "Only one OAuth provider can be configured".to_string(),
                                ),
                            ));
                        }

                        let (oauth_client, oauth_identity_provider) = match &oauth_config {
                            #[cfg(feature = "oauth-github")]
                            OAuthConfig::GitHub {
                                client_id,
                                client_secret,
                                redirect_url,
                            } => (
                                OAuthClient::new_github(
                                    client_id.to_string(),
                                    client_secret.to_string(),
                                    redirect_url.to_string(),
                                )
                                .map_err(|err| {
                                    RestApiServerError::InvalidStateError(
                                        InvalidStateError::with_message(format!(
                                            "Invalid GitHub OAuth config provided: {}",
                                            err
                                        )),
                                    )
                                })?,
                                Box::new(GithubUserIdentityProvider),
                            ),
                        };
                        // Save user information, including tokens
                        let save_user_info_operation: Box<dyn SaveUserInfoOperation> =
                            match user_info_save_config {
                                #[cfg(feature = "biome-oauth")]
                                UserInfoSaveConfig::Biome {
                                    user_store,
                                    oauth_user_store,
                                } => {
                                    let oauth_provider = match oauth_config {
                                        #[cfg(feature = "oauth-github")]
                                        OAuthConfig::GitHub { .. } => OAuthProvider::Github,
                                    };
                                    // Add the configuration mapping for the User value.
                                    self.authorization_mappings.push(
                                        ConfigureAuthorizationMapping::new(
                                            GetUserByOAuthAuthorization::new(
                                                oauth_user_store.clone(),
                                            ),
                                        ),
                                    );
                                    Box::new(OAuthUserStoreSaveUserInfoOperation::new(
                                        oauth_provider,
                                        user_store,
                                        oauth_user_store,
                                    ))
                                }
                                UserInfoSaveConfig::NoOp => Box::new(SaveUserInfoToNull),
                            };

                        identity_providers.push(oauth_identity_provider);
                        self.resources.append(
                            &mut OAuthResourceProvider::new(oauth_client, save_user_info_operation)
                                .resources(),
                        );
                        oauth_configured = true;
                    }
                    AuthConfig::Custom {
                        mut resources,
                        identity_provider,
                    } => {
                        self.resources.append(&mut resources);
                        identity_providers.push(identity_provider);
                    }
                }
            }

            identity_providers
        };

        Ok(RestApi {
            bind,
            resources: self.resources,
            #[cfg(feature = "rest-api-cors")]
            whitelist: self.whitelist,
            #[cfg(feature = "auth")]
            identity_providers,
            #[cfg(feature = "auth")]
            authorization_mappings: self.authorization_mappings,
        })
    }

    /// Builds the `RestApi` without requiring any security configuration
    #[cfg(test)]
    pub fn build_insecure(self) -> Result<RestApi, RestApiServerError> {
        let bind = self
            .bind
            .ok_or_else(|| RestApiServerError::MissingField("bind".to_string()))?;

        Ok(RestApi {
            bind,
            resources: self.resources,
            #[cfg(feature = "rest-api-cors")]
            whitelist: self.whitelist,
            #[cfg(feature = "auth")]
            identity_providers: vec![],
            #[cfg(feature = "auth")]
            authorization_mappings: self.authorization_mappings,
        })
    }
}

/// Configurations for the various authentication methods supported by the Splinter REST API.
#[cfg(feature = "auth")]
pub enum AuthConfig {
    /// Biome credentials authentication
    #[cfg(feature = "biome-credentials")]
    Biome {
        /// The resource provider that defines all Biome-related endpoints for the Splinter REST API
        biome_resource_manager: BiomeRestResourceManager,
    },
    /// Cylinder JWT authentication
    #[cfg(feature = "cylinder-jwt")]
    Cylinder {
        /// The signature verifier used to validate Cylinder JWTs
        verifier: Box<dyn Verifier>,
    },
    /// OAuth authentication
    #[cfg(feature = "oauth")]
    OAuth {
        /// OAuth provider configuration
        oauth_config: OAuthConfig,
        /// The configuration for the user info save operation
        user_info_save_config: UserInfoSaveConfig,
    },
    /// A custom authentication method
    Custom {
        /// REST API resources that would allow a client to receive some authentication credentials
        resources: Vec<Resource>,
        /// The identity provider that correlates the contents of the `Authorization` header with
        /// an identity for the client
        identity_provider: Box<dyn IdentityProvider>,
    },
}

/// OAuth configurations that are supported out-of-the-box by the Splinter REST API.
#[cfg(feature = "oauth")]
pub enum OAuthConfig {
    /// OAuth provided by GitHub
    #[cfg(feature = "oauth-github")]
    GitHub {
        /// The client ID of the GitHub OAuth app
        client_id: String,
        /// The client secret of the GitHub OAuth app
        client_secret: String,
        /// The redirect URL that is configured for the GitHub OAuth app
        redirect_url: String,
    },
}

/// Configurations for how users' information, including tokens and identity, are saved when
/// they're received from the OAuth provider
#[cfg(feature = "oauth")]
pub enum UserInfoSaveConfig {
    /// Saves users' information to Biome's OAuth user store
    #[cfg(feature = "biome-oauth")]
    Biome {
        user_store: Box<dyn UserStore>,
        oauth_user_store: Box<dyn OAuthUserStore>,
    },
    /// Users' information will not be saved by the Splinter REST API
    NoOp,
}

pub fn into_protobuf<M: Message>(
    payload: web::Payload,
) -> impl Future<Item = M, Error = ActixError> {
    payload
        .from_err::<ActixError>()
        .fold(web::BytesMut::new(), move |mut body, chunk| {
            body.extend_from_slice(&chunk);
            Ok::<_, ActixError>(body)
        })
        .and_then(|body| match protobuf::parse_from_bytes::<M>(&body) {
            Ok(proto) => Ok(proto),
            Err(err) => Err(ErrorBadRequest(json!({ "message": format!("{}", err) }))),
        })
        .into_future()
}

pub fn into_bytes(payload: web::Payload) -> impl Future<Item = Vec<u8>, Error = ActixError> {
    payload
        .from_err::<ActixError>()
        .fold(web::BytesMut::new(), move |mut body, chunk| {
            body.extend_from_slice(&chunk);
            Ok::<_, ActixError>(body)
        })
        .and_then(|body| Ok(body.to_vec()))
        .into_future()
}

pub fn percent_encode_filter_query(input: &str) -> String {
    percent_encoding::utf8_percent_encode(input, QUERY_ENCODE_SET).to_string()
}

pub fn require_header(header_key: &str, request: &HttpRequest) -> Result<String, RequestError> {
    let header = request.headers().get(header_key).ok_or_else(|| {
        RequestError::MissingHeader(format!("Header {} not included in Request", header_key))
    })?;
    Ok(header
        .to_str()
        .map_err(|err| RequestError::InvalidHeaderValue(format!("Invalid header value: {}", err)))?
        .to_string())
}

pub fn get_authorization_token(request: &HttpRequest) -> Result<String, RequestError> {
    let auth_header = require_header("Authorization", &request)?;
    Ok(auth_header
        .split_whitespace()
        .last()
        .ok_or_else(|| {
            RequestError::InvalidHeaderValue(
                "Authorization token not included in request".to_string(),
            )
        })?
        .to_string())
}

#[cfg(test)]
mod test {
    use super::*;
    use actix_http::Response;
    use futures::IntoFuture;

    #[cfg(feature = "auth")]
    use crate::auth::rest_api::identity::Authorization;
    #[cfg(feature = "auth")]
    use crate::error::InternalError;

    #[test]
    fn test_resource() {
        Resource::build("/test")
            .add_method(Method::Get, |_: HttpRequest, _: web::Payload| {
                Box::new(Response::Ok().finish().into_future())
            })
            .into_route();
    }

    #[test]
    fn test_resource_with_guard() {
        Resource::build("/test-guarded")
            .add_request_guard(|_: &HttpRequest| {
                Continuation::terminate(Response::BadRequest().finish().into_future())
            })
            .add_method(Method::Get, |_: HttpRequest, _: web::Payload| {
                Box::new(Response::Ok().finish().into_future())
            })
            .into_route();
    }

    /// Verifies that the `RestApiBuilder` builds succesfully when all required configuration is
    /// provided.
    #[test]
    fn rest_api_builder_successful() {
        // Allowing unused_mut because builder must be mutable if feature auth is enabled
        #[allow(unused_mut)]
        let mut builder = RestApiBuilder::new().with_bind("test");

        #[cfg(feature = "auth")]
        {
            let auth_config = AuthConfig::Custom {
                resources: vec![],
                identity_provider: Box::new(MockIdentityProvider),
            };
            builder = builder.with_auth_configs(vec![auth_config]);
        }

        assert!(builder.build().is_ok())
    }

    /// Verifies that the `RestApiBuilder` fails to build when auth is enabled but no auth is
    /// configured.
    #[test]
    #[cfg(feature = "auth")]
    fn rest_api_builder_no_auth() {
        assert!(matches!(
            RestApiBuilder::new().with_bind("test").build(),
            Err(RestApiServerError::InvalidStateError(_))
        ));
    }

    #[cfg(feature = "auth")]
    #[derive(Clone)]
    struct MockIdentityProvider;

    #[cfg(feature = "auth")]
    impl IdentityProvider for MockIdentityProvider {
        fn get_identity(
            &self,
            _authorization: &Authorization,
        ) -> Result<Option<String>, InternalError> {
            Ok(Some("".into()))
        }

        /// Clones implementation for `IdentityProvider`. The implementation of the `Clone` trait for
        /// `Box<dyn IdentityProvider>` calls this method.
        ///
        /// # Example
        ///
        ///```ignore
        ///  fn clone_box(&self) -> Box<dyn IdentityProvider> {
        ///     Box::new(self.clone())
        ///  }
        ///```
        fn clone_box(&self) -> Box<dyn IdentityProvider> {
            Box::new(self.clone())
        }
    }
}
