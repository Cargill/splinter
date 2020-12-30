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

//! Authorization middleware for the Actix REST API

use actix_web::dev::*;
use actix_web::{
    http::{
        header::{self, HeaderValue},
        Method as ActixMethod,
    },
    Error as ActixError, HttpMessage, HttpResponse,
};
use futures::{
    future::{ok, FutureResult},
    Future, IntoFuture, Poll,
};

use crate::rest_api::ErrorResponse;
#[cfg(feature = "authorization")]
use crate::rest_api::Method;

#[cfg(feature = "authorization")]
use super::PermissionMap;
use super::{authorize, identity::IdentityProvider, AuthorizationResult};

/// Wrapper for the authorization middleware
#[derive(Clone)]
pub struct Authorization {
    identity_providers: Vec<Box<dyn IdentityProvider>>,
}

impl Authorization {
    pub fn new(identity_providers: Vec<Box<dyn IdentityProvider>>) -> Self {
        Self { identity_providers }
    }
}

impl<S, B> Transform<S> for Authorization
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = ActixError>,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = S::Error;
    type InitError = ();
    type Transform = AuthorizationMiddleware<S>;
    type Future = FutureResult<Self::Transform, Self::InitError>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(AuthorizationMiddleware {
            identity_providers: self.identity_providers.clone(),
            service,
        })
    }
}

/// Authorization middleware for the Actix REST API
pub struct AuthorizationMiddleware<S> {
    identity_providers: Vec<Box<dyn IdentityProvider>>,
    service: S,
}

impl<S, B> Service for AuthorizationMiddleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = ActixError>,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = S::Error;
    type Future = Box<dyn Future<Item = Self::Response, Error = Self::Error>>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        self.service.poll_ready()
    }

    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        if req.method() == ActixMethod::OPTIONS {
            return Box::new(self.service.call(req).and_then(|mut res| {
                res.headers_mut().insert(
                    header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                    HeaderValue::from_static("true"),
                );

                res
            }));
        }

        #[cfg(feature = "authorization")]
        let method = match *req.method() {
            ActixMethod::GET => Method::Get,
            ActixMethod::POST => Method::Post,
            ActixMethod::PUT => Method::Put,
            ActixMethod::PATCH => Method::Patch,
            ActixMethod::DELETE => Method::Delete,
            ActixMethod::HEAD => Method::Head,
            _ => {
                return Box::new(
                    req.into_response(
                        HttpResponse::BadRequest()
                            .json(ErrorResponse::bad_request(
                                "HTTP method not supported by Splinter REST API",
                            ))
                            .into_body(),
                    )
                    .into_future(),
                )
            }
        };

        let auth_header =
            match req
                .headers()
                .get("Authorization")
                .map(|auth| auth.to_str())
                .transpose()
            {
                Ok(opt) => opt,
                // Not including the error since it could leak secrets from the Authorization header
                Err(_) => return Box::new(
                    req.into_response(
                        HttpResponse::BadRequest()
                            .json(ErrorResponse::bad_request(
                                "Authorization header must contain only visible ASCII characters",
                            ))
                            .into_body(),
                    )
                    .into_future(),
                ),
            };

        #[cfg(feature = "authorization")]
        let permission_map = match req.app_data::<PermissionMap>() {
            Some(map) => map,
            None => {
                error!("Missing REST API permission map");
                return Box::new(
                    req.into_response(
                        HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error())
                            .into_body(),
                    )
                    .into_future(),
                );
            }
        };

        match authorize(
            #[cfg(feature = "authorization")]
            &method,
            req.path(),
            auth_header,
            #[cfg(feature = "authorization")]
            permission_map.get_ref(),
            &self.identity_providers,
        ) {
            AuthorizationResult::Authorized(identity) => {
                debug!("Authenticated user {:?}", identity);
                req.extensions_mut().insert(identity);
            }
            AuthorizationResult::NoAuthorizationNecessary => {}
            AuthorizationResult::Unauthorized => {
                return Box::new(
                    req.into_response(
                        HttpResponse::Unauthorized()
                            .json(ErrorResponse::unauthorized())
                            .into_body(),
                    )
                    .into_future(),
                )
            }
            AuthorizationResult::UnknownEndpoint => {
                return Box::new(
                    req.into_response(
                        HttpResponse::NotFound()
                            .json(ErrorResponse::not_found("endpoint not found"))
                            .into_body(),
                    )
                    .into_future(),
                )
            }
        }

        Box::new(self.service.call(req).and_then(|mut res| {
            res.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                HeaderValue::from_static("true"),
            );

            res
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use actix_web::{http::StatusCode, test, web, App, HttpRequest};

    use crate::error::InternalError;
    #[cfg(feature = "authorization")]
    use crate::rest_api::auth::Permission;
    use crate::rest_api::auth::{identity::Identity, AuthorizationHeader};

    /// Verifies that the authorization middleware sets the `Access-Control-Allow-Credentials: true`
    /// header for `OPTIONS` requests.
    #[test]
    fn auth_middleware_options_request_header() {
        let mut app = test::init_service(
            App::new().wrap(Authorization::new(vec![])).route(
                "/",
                web::route()
                    .method(ActixMethod::OPTIONS)
                    .to(|| HttpResponse::Ok()),
            ),
        );

        let req = test::TestRequest::with_uri("/")
            .method(ActixMethod::OPTIONS)
            .to_request();
        let resp = test::block_on(app.call(req)).unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get("Access-Control-Allow-Credentials"),
            Some(&HeaderValue::from_static("true"))
        );
    }

    /// Verifies that the authorization middleware returns a `403 Unauthorized` response when the
    /// `authorize` function returns an "unauthorized" result. This is simulated by not configuring
    /// any identity providers.
    #[test]
    fn auth_middleware_unauthorized() {
        let app = App::new()
            .wrap(Authorization::new(vec![]))
            .route("/", web::get().to(|| HttpResponse::Ok()));

        #[cfg(feature = "authorization")]
        let app = {
            let mut permission_map = PermissionMap::default();
            permission_map.add_permission(Method::Get, "/", Permission::AllowAuthenticated);
            app.data(permission_map)
        };

        let mut service = test::init_service(app);

        let req = test::TestRequest::with_uri("/").to_request();
        let resp = test::block_on(service.call(req)).unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    /// Verifies that the authorization middleware returns a `404 Not Found` response when the
    /// `authorize` function returns an "unkonwn endpoint" result.
    #[test]
    fn auth_middleware_not_found() {
        let app = App::new()
            .wrap(Authorization::new(vec![]))
            .route("/", web::get().to(|| HttpResponse::Ok()));

        #[cfg(feature = "authorization")]
        let app = {
            let mut permission_map = PermissionMap::default();
            permission_map.add_permission(Method::Get, "/", Permission::AllowAuthenticated);
            app.data(permission_map)
        };

        let mut service = test::init_service(app);

        let req = test::TestRequest::with_uri("/test").to_request();
        let resp = test::block_on(service.call(req)).unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    /// Verifies that the authorization middleware allows requests that are properly authorized (the
    /// `authorize` function returns an "authorized" result), and that the client's identity is
    /// added to the request's extensions.
    #[test]
    fn auth_middleware_authorized() {
        let auth_middleware = Authorization::new(vec![Box::new(AlwaysAcceptIdentityProvider)]);

        let app = App::new().wrap(auth_middleware).route(
            "/",
            web::get().to(|req: HttpRequest| {
                // Verify that the client's identity was added to the request extensions
                if req.extensions().get() == Some(&Identity::Custom("identity".into())) {
                    HttpResponse::Ok()
                } else {
                    HttpResponse::InternalServerError()
                }
            }),
        );

        #[cfg(feature = "authorization")]
        let app = {
            let mut permission_map = PermissionMap::default();
            permission_map.add_permission(Method::Get, "/", Permission::AllowAuthenticated);
            app.data(permission_map)
        };

        let mut service = test::init_service(app);

        // Need to provide some value for the `Authorization` header
        let req = test::TestRequest::with_uri("/")
            .header("Authorization", "test")
            .to_request();
        let resp = test::block_on(service.call(req)).unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
    }

    /// An identity provider that always returns `Ok(Some(_))`
    #[derive(Clone)]
    struct AlwaysAcceptIdentityProvider;

    impl IdentityProvider for AlwaysAcceptIdentityProvider {
        fn get_identity(
            &self,
            _authorization: &AuthorizationHeader,
        ) -> Result<Option<Identity>, InternalError> {
            Ok(Some(Identity::Custom("identity".into())))
        }

        fn clone_box(&self) -> Box<dyn IdentityProvider> {
            Box::new(self.clone())
        }
    }
}
