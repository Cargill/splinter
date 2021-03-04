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

use std::sync::Arc;

use actix_web::{
    error::ErrorBadRequest, http::header, web, Error as ActixError, HttpRequest, HttpResponse,
};
use futures::{future::IntoFuture, stream::Stream, Future};
use protobuf::{self, Message};

#[cfg(feature = "authorization")]
use crate::rest_api::auth::authorization::{Permission, PermissionMap};

use super::{Continuation, RequestGuard};

/// Rest methods compatible with `RestApi`.
#[derive(Clone, PartialEq)]
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

pub fn into_protobuf<M: Message>(
    payload: web::Payload,
) -> impl Future<Item = M, Error = ActixError> {
    payload
        .from_err::<ActixError>()
        .fold(web::BytesMut::new(), move |mut body, chunk| {
            body.extend_from_slice(&chunk);
            Ok::<_, ActixError>(body)
        })
        .and_then(|body| match Message::parse_from_bytes(&body) {
            Ok(proto) => Ok(proto),
            Err(err) => Err(ErrorBadRequest(json!({ "message": format!("{}", err) }))),
        })
        .into_future()
}

pub type HandlerFunction = Box<
    dyn Fn(HttpRequest, web::Payload) -> Box<dyn Future<Item = HttpResponse, Error = ActixError>>
        + Send
        + Sync
        + 'static,
>;

#[cfg(feature = "authorization")]
#[derive(Clone)]
struct ResourceMethod {
    method: Method,
    permission: Permission,
    handler: Arc<HandlerFunction>,
}

/// `Resource` represents a RESTful endpoint.
///
/// ```
/// use actix_web::HttpResponse;
/// use futures::IntoFuture;
/// use splinter::rest_api::{Resource, Method, auth::authorization::Permission};
///
/// Resource::build("/index")
///     .add_method(Method::Get, Permission::AllowUnauthenticated, |r, p| {
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
    #[cfg(feature = "authorization")]
    methods: Vec<ResourceMethod>,
    #[cfg(not(feature = "authorization"))]
    methods: Vec<(Method, Arc<HandlerFunction>)>,
}

impl Resource {
    #[cfg(not(feature = "authorization"))]
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

    #[cfg(feature = "authorization")]
    pub fn add_method<F>(mut self, method: Method, permission: Permission, handle: F) -> Self
    where
        F: Fn(
                HttpRequest,
                web::Payload,
            ) -> Box<dyn Future<Item = HttpResponse, Error = ActixError>>
            + Send
            + Sync
            + 'static,
    {
        self.methods.push(ResourceMethod {
            method,
            permission,
            handler: Arc::new(Box::new(handle)),
        });
        self
    }

    #[cfg(not(feature = "authorization"))]
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
    /// use actix_web::{HttpRequest, HttpResponse};
    /// use futures::IntoFuture;
    /// use splinter::rest_api::{Resource, Method, Continuation, auth::authorization::Permission};
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
    ///     .add_method(Method::Get, Permission::AllowUnauthenticated, |r, p| {
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

    #[cfg(feature = "authorization")]
    pub(super) fn into_route(self) -> (actix_web::Resource, PermissionMap) {
        let mut resource = web::resource(&self.route);

        let mut allowed_methods = self
            .methods
            .iter()
            .map(|resource_method| resource_method.method.to_string())
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
        let mut permission_map = PermissionMap::new();
        let route = self.route.clone();
        let resource = self.methods.into_iter().fold(
            resource,
            |resource,
             ResourceMethod {
                 method,
                 permission,
                 handler,
             }| {
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
                permission_map.add_permission(method.clone(), &route, permission);
                resource.route(match method {
                    Method::Get => web::get().to_async(func),
                    Method::Post => web::post().to_async(func),
                    Method::Put => web::put().to_async(func),
                    Method::Patch => web::patch().to_async(func),
                    Method::Delete => web::delete().to_async(func),
                    Method::Head => web::head().to_async(func),
                })
            },
        );
        (resource, permission_map)
    }

    #[cfg(not(feature = "authorization"))]
    pub(super) fn into_route(self) -> actix_web::Resource {
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

/// A `RestResourceProvider` provides a list of resources that are consumed by `RestApi`.
pub trait RestResourceProvider {
    fn resources(&self) -> Vec<Resource>;
}

#[cfg(test)]
mod test {
    use super::*;
    use actix_http::Response;
    use futures::IntoFuture;

    #[test]
    fn test_resource() {
        let mut resource = Resource::build("/test");
        #[cfg(feature = "authorization")]
        {
            resource = resource.add_method(
                Method::Get,
                Permission::AllowUnauthenticated,
                |_: HttpRequest, _: web::Payload| Box::new(Response::Ok().finish().into_future()),
            );
        }
        #[cfg(not(feature = "authorization"))]
        {
            resource = resource.add_method(Method::Get, |_: HttpRequest, _: web::Payload| {
                Box::new(Response::Ok().finish().into_future())
            });
        }
        resource.into_route();
    }

    #[test]
    fn test_resource_with_guard() {
        let mut resource = Resource::build("/test-guarded").add_request_guard(|_: &HttpRequest| {
            Continuation::terminate(Response::BadRequest().finish().into_future())
        });
        #[cfg(feature = "authorization")]
        {
            resource = resource.add_method(
                Method::Get,
                Permission::AllowUnauthenticated,
                |_: HttpRequest, _: web::Payload| Box::new(Response::Ok().finish().into_future()),
            );
        }
        #[cfg(not(feature = "authorization"))]
        {
            resource = resource.add_method(Method::Get, |_: HttpRequest, _: web::Payload| {
                Box::new(Response::Ok().finish().into_future())
            });
        }
        resource.into_route();
    }

    #[cfg(feature = "authorization")]
    #[test]
    fn test_resource_permission() {
        let permission = Permission::Check {
            permission_id: "test",
            permission_display_name: "",
            permission_description: "",
        };
        let (_, permission_map) = Resource::build("/test")
            .add_method(
                Method::Get,
                permission,
                |_: HttpRequest, _: web::Payload| Box::new(Response::Ok().finish().into_future()),
            )
            .into_route();

        assert_eq!(
            permission_map
                .get_permission(&Method::Get, "/test")
                .expect("Missing permission"),
            &permission,
        );
    }
}
