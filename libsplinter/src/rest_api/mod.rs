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
//! ```
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
//!         vec![Resource::build("/index").add_method(Method::Get, move |r| {
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

#[cfg(feature = "rest-api-actix")]
mod actix_impl;
#[cfg(feature = "rest-api-cors")]
pub mod cors;
mod errors;
mod events;
pub mod paging;
mod response_models;
#[cfg(feature = "json-web-tokens")]
pub mod secrets;
#[cfg(feature = "json-web-tokens")]
pub mod sessions;

use std::boxed::Box;
use std::collections::BTreeMap;
use std::sync::{mpsc, Arc};
use std::thread;

#[cfg(feature = "rest-api-actix")]
use actix_web::HttpRequest as ActixRequest;
use actix_web::{
    http::header, middleware, web, App, Error as ActixError, HttpResponse, HttpServer,
};
use futures::{Future, IntoFuture};
use percent_encoding::{AsciiSet, CONTROLS};

use errors::RequestBuilderError;
pub use errors::{RestApiServerError, WebSocketError};
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
    dyn Fn(Request) -> Box<dyn Future<Item = HttpResponse, Error = ActixError>>
        + Send
        + Sync
        + 'static,
>;

#[derive(Clone, Debug)]
pub struct Request {
    path: String,
    headers: BTreeMap<String, String>,
    path_parameters: BTreeMap<String, String>,
    query_parameters: BTreeMap<String, String>,
    body: Vec<u8>,
    // This field is required by the Actix implementation of WebSocket subscription, since the
    // `actix_web_actors::ws::start` function takes the Actix request as an argument.
    #[cfg(feature = "rest-api-actix")]
    actix_request: ActixRequest,
}

impl Request {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn headers(&self) -> &BTreeMap<String, String> {
        &self.headers
    }

    pub fn header(&self, header: &str) -> Option<&str> {
        self.headers.get(header).map(String::as_str)
    }

    pub fn path_parameters(&self) -> &BTreeMap<String, String> {
        &self.path_parameters
    }

    pub fn path_parameter(&self, param: &str) -> Option<&str> {
        self.path_parameters.get(param).map(String::as_str)
    }

    pub fn query_parameters(&self) -> &BTreeMap<String, String> {
        &self.query_parameters
    }

    pub fn query_parameter(&self, param: &str) -> Option<&str> {
        self.query_parameters.get(param).map(String::as_str)
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }

    #[cfg(feature = "rest-api-actix")]
    pub fn actix_request(&self) -> &ActixRequest {
        &self.actix_request
    }
}

#[derive(Default)]
struct RequestBuilder {
    path: Option<String>,
    headers: Option<BTreeMap<String, String>>,
    path_parameters: Option<BTreeMap<String, String>>,
    query_parameters: Option<BTreeMap<String, String>>,
    body: Option<Vec<u8>>,
    #[cfg(feature = "rest-api-actix")]
    actix_request: Option<ActixRequest>,
}

impl RequestBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_path(mut self, path: String) -> Self {
        self.path = Some(path);
        self
    }

    pub fn with_headers(mut self, headers: BTreeMap<String, String>) -> Self {
        self.headers = Some(headers);
        self
    }

    pub fn with_path_parameters(mut self, path_parameters: BTreeMap<String, String>) -> Self {
        self.path_parameters = Some(path_parameters);
        self
    }

    pub fn with_query_parameters(mut self, query_parameters: BTreeMap<String, String>) -> Self {
        self.query_parameters = Some(query_parameters);
        self
    }

    pub fn with_body(mut self, body: Vec<u8>) -> Self {
        self.body = Some(body);
        self
    }

    #[cfg(feature = "rest-api-actix")]
    pub fn with_actix_request(mut self, actix_request: ActixRequest) -> Self {
        self.actix_request = Some(actix_request);
        self
    }

    pub fn build(self) -> Result<Request, RequestBuilderError> {
        Ok(Request {
            path: self
                .path
                .ok_or_else(|| RequestBuilderError::MissingRequiredField("path".into()))?,
            headers: self.headers.unwrap_or_default(),
            path_parameters: self.path_parameters.unwrap_or_default(),
            query_parameters: self.query_parameters.unwrap_or_default(),
            body: self.body.unwrap_or_default(),
            #[cfg(feature = "rest-api-actix")]
            actix_request: self
                .actix_request
                .ok_or_else(|| RequestBuilderError::MissingRequiredField("actix_request".into()))?,
        })
    }
}

/// Shutdown handle returned by `RestApi::run`. Allows rest api instance to be shut down
/// gracefully.
pub struct RestApiShutdownHandle {
    do_shutdown: Box<dyn Fn() -> Result<(), RestApiServerError> + Send>,
}

impl RestApiShutdownHandle {
    pub fn shutdown(&self) -> Result<(), RestApiServerError> {
        (*self.do_shutdown)()
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
///     .add_method(Method::Get, |r| {
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
        F: Fn(Request) -> Box<dyn Future<Item = HttpResponse, Error = ActixError>>
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
        F: Fn(Request) -> Box<dyn Future<Item = HttpResponse, Error = ActixError>>
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
    /// use splinter::rest_api::{Request, Resource, Method, Continuation};
    /// use actix_web::HttpResponse;
    /// use futures::IntoFuture;
    ///
    /// Resource::build("/index")
    ///     .add_request_guard(|r: &Request| {
    ///         if !r.headers().contains_key("GuardFlag") {
    ///             Continuation::terminate(
    ///                 HttpResponse::BadRequest().finish().into_future(),
    ///             )
    ///         } else {
    ///             Continuation::Continue
    ///         }
    ///     })
    ///     .add_method(Method::Get, |r| {
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
            move |_: Request| {
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
                let func = move |r: Request| {
                    // This clone satisfies a requirement that this be FnOnce
                    if !guards.is_empty() {
                        for guard in guards.clone() {
                            match guard.evaluate(&r) {
                                Continuation::Terminate(result) => return result,
                                Continuation::Continue => (),
                            }
                        }
                    }
                    (handler)(r)
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
    fn evaluate(&self, req: &Request) -> Continuation;
}

impl<F> RequestGuard for F
where
    F: Fn(&Request) -> Continuation + Sync + Send,
{
    fn evaluate(&self, req: &Request) -> Continuation {
        (*self)(req)
    }
}

impl RequestGuard for Box<dyn RequestGuard> {
    fn evaluate(&self, req: &Request) -> Continuation {
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
    fn evaluate(&self, req: &Request) -> Continuation {
        if let Some(header_value) = req.header("SplinterProtocolVersion") {
            let parsed_header = header_value.parse::<u32>().map_err(|_| {
                "SplinterProtocolVersion must be a valid positive integer".to_string()
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

/// `RestApi` is used to create an instance of a restful web server.
#[derive(Clone)]
pub struct RestApi {
    resources: Vec<Resource>,
    bind: String,
}

impl RestApi {
    pub fn run(
        self,
    ) -> Result<(RestApiShutdownHandle, thread::JoinHandle<()>), RestApiServerError> {
        let (tx, rx) = mpsc::channel();

        let bind_url = self.bind.to_owned();
        let resources = self.resources.to_owned();
        let join_handle = thread::Builder::new()
            .name("SplinterDRestApi".into())
            .spawn(move || {
                let sys = actix::System::new("SplinterD-Rest-API");
                let mut server = HttpServer::new(move || {
                    // Actix's type definitions require this to be chained, otherwise, the generic
                    // type of App is changed as the values are returned.
                    #[cfg(feature = "rest-api-cors")]
                    let mut app = App::new()
                        .wrap(middleware::Logger::default())
                        .wrap(cors::Cors::new_allow_any());

                    #[cfg(not(feature = "rest-api-cors"))]
                    let mut app = App::new().wrap(middleware::Logger::default());

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

                let addr = server.disable_signals().system_exit().start();

                if let Err(err) = tx.send(Ok(addr)) {
                    error!("Unable to send Server Addr: {}", err);
                }

                if let Err(err) = sys.run() {
                    error!("REST Api unexpectedly exiting: {}", err);
                };

                info!("Rest API terminating");
            })?;

        let addr = rx
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

        Ok((RestApiShutdownHandle { do_shutdown }, join_handle))
    }
}

/// Builder `struct` for `RestApi`.
pub struct RestApiBuilder {
    resources: Vec<Resource>,
    bind: Option<String>,
}

impl Default for RestApiBuilder {
    fn default() -> Self {
        Self {
            resources: Vec::new(),
            bind: None,
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

    pub fn build(self) -> Result<RestApi, RestApiServerError> {
        let bind = self
            .bind
            .ok_or_else(|| RestApiServerError::MissingField("bind".to_string()))?;

        Ok(RestApi {
            bind,
            resources: self.resources,
        })
    }
}

pub fn percent_encode_filter_query(input: &str) -> String {
    percent_encoding::utf8_percent_encode(input, QUERY_ENCODE_SET).to_string()
}

#[cfg(test)]
mod test {
    use super::*;
    use actix_http::Response;
    use futures::IntoFuture;

    #[test]
    fn test_resource() {
        Resource::build("/test")
            .add_method(Method::Get, |_: Request| {
                Box::new(Response::Ok().finish().into_future())
            })
            .into_route();
    }

    #[test]
    fn test_resource_with_guard() {
        Resource::build("/test-guarded")
            .add_request_guard(|_: &Request| {
                Continuation::terminate(Response::BadRequest().finish().into_future())
            })
            .add_method(Method::Get, |_: Request| {
                Box::new(Response::Ok().finish().into_future())
            })
            .into_route();
    }
}
