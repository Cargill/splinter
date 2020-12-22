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

use std::sync::Arc;

use actix_web::dev::*;
use actix_web::{
    http::{
        header::{self, HeaderValue},
        Method,
    },
    Error as ActixError, HttpMessage, HttpResponse,
};
use futures::{
    future::{ok, FutureResult},
    Future, IntoFuture, Poll,
};

use crate::error::InternalError;
use crate::rest_api::ErrorResponse;

use super::{
    authorize, identity::IdentityProvider, AuthorizationHeader, AuthorizationMapping,
    AuthorizationResult,
};

/// Wrapper for the authorization middleware
#[derive(Clone)]
pub struct Authorization {
    identity_providers: Vec<Box<dyn IdentityProvider>>,
    identity_extensions: Vec<Arc<IdentityExtension>>,
}

/// This is a wrapper to avoid multiple generic types.
struct IdentityExtension {
    inner: Box<
        dyn Fn(&mut Extensions, &AuthorizationHeader) -> Result<(), InternalError> + Send + Sync,
    >,
}

impl IdentityExtension {
    /// Wrap a AuthorizationMapping implementation in an IdentityExtension, to provide the ability to
    /// add a value from the AuthorizationMapping trait to the HttpRequest.
    fn new<G, T>(get_by_auth: G) -> Self
    where
        T: 'static,
        G: AuthorizationMapping<T> + Send + Sync + 'static,
    {
        Self {
            inner: Box::new(move |extensions, identity_auth| {
                debug!("executing auth mapping {}", std::any::type_name::<G>());
                if let Some(xformed) = get_by_auth.get(identity_auth)? {
                    extensions.insert(xformed);
                }

                Ok(())
            }),
        }
    }

    /// Extend the given HttpRequest::Extensions.
    fn extend(
        &self,
        extensions: &mut Extensions,
        authorization: &AuthorizationHeader,
    ) -> Result<(), InternalError> {
        (*self.inner)(extensions, authorization)
    }
}

impl Authorization {
    pub fn new(identity_providers: Vec<Box<dyn IdentityProvider>>) -> Self {
        Self {
            identity_providers,
            identity_extensions: Vec::new(),
        }
    }

    /// Add an authorization mapping, provided by a AuthorizationMapping implementation.
    pub fn with_authorization_mapping<M, T>(mut self, auth_mapping: M) -> Self
    where
        T: 'static,
        M: AuthorizationMapping<T> + Send + Sync + 'static,
    {
        self.identity_extensions
            .push(Arc::new(IdentityExtension::new(auth_mapping)));

        self
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
            identity_extensions: self.identity_extensions.clone(),
            service,
        })
    }
}

/// Authorization middleware for the Actix REST API
pub struct AuthorizationMiddleware<S> {
    identity_providers: Vec<Box<dyn IdentityProvider>>,
    identity_extensions: Vec<Arc<IdentityExtension>>,
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
        if req.method() == Method::OPTIONS {
            return Box::new(self.service.call(req).and_then(|mut res| {
                res.headers_mut().insert(
                    header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                    HeaderValue::from_static("true"),
                );

                res
            }));
        }

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

        match authorize(req.path(), auth_header, &self.identity_providers) {
            AuthorizationResult::Authorized {
                authorization,
                identity,
            } => {
                for identity_extension in self.identity_extensions.iter() {
                    let res =
                        { identity_extension.extend(&mut req.extensions_mut(), &authorization) };

                    if let Err(err) = res {
                        error!("Unable to transform extension: {}", err);
                        return Box::new(
                            req.into_response(
                                HttpResponse::InternalServerError()
                                    .json(ErrorResponse::internal_error())
                                    .into_body(),
                            )
                            .into_future(),
                        );
                    }
                }
                debug!("Authenticated user {}", identity);
            }
            #[cfg(any(feature = "biome-credentials", feature = "oauth"))]
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

    /// Verifies that the authorization middleware sets the `Access-Control-Allow-Credentials: true`
    /// header for `OPTIONS` requests.
    #[test]
    fn auth_middleware_options_request_header() {
        let mut app = test::init_service(
            App::new().wrap(Authorization::new(vec![])).route(
                "/",
                web::route()
                    .method(Method::OPTIONS)
                    .to(|| HttpResponse::Ok()),
            ),
        );

        let req = test::TestRequest::with_uri("/")
            .method(Method::OPTIONS)
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
    /// and identity providers.
    #[test]
    fn auth_middleware_unauthorized() {
        let mut app = test::init_service(
            App::new()
                .wrap(Authorization::new(vec![]))
                .route("/", web::get().to(|| HttpResponse::Ok())),
        );

        let req = test::TestRequest::with_uri("/").to_request();
        let resp = test::block_on(app.call(req)).unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    /// Verifies that the authorization middleware allows requests that are properly authorized (the
    /// `authorize` function returns an "authorized" result), and that
    /// `AuthorizationMapping`s/`IdentityExtension`s are properly applied.
    #[test]
    fn auth_middleware_authorized() {
        let auth_middleware = Authorization::new(vec![Box::new(AlwaysAcceptIdentityProvider)])
            .with_authorization_mapping(MockAuthorizationMapping);

        let mut app = test::init_service(App::new().wrap(auth_middleware).route(
            "/",
            web::get().to(|req: HttpRequest| {
                // Verify that the expected string was added to the request extensions by the
                // `MockAuthorizationMapping`
                if req.extensions().get() == Some(&"test".to_string()) {
                    HttpResponse::Ok()
                } else {
                    HttpResponse::InternalServerError()
                }
            }),
        ));

        // Need to provide some value for the `Authorization` header
        let req = test::TestRequest::with_uri("/")
            .header("Authorization", "test")
            .to_request();
        let resp = test::block_on(app.call(req)).unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
    }

    /// An identity provider that always returns `Ok(Some("identity"))`
    #[derive(Clone)]
    struct AlwaysAcceptIdentityProvider;

    impl IdentityProvider for AlwaysAcceptIdentityProvider {
        fn get_identity(
            &self,
            _authorization: &AuthorizationHeader,
        ) -> Result<Option<String>, InternalError> {
            Ok(Some("identity".into()))
        }

        fn clone_box(&self) -> Box<dyn IdentityProvider> {
            Box::new(self.clone())
        }
    }

    /// An `AuthorizationMapping` that just returns a string
    struct MockAuthorizationMapping;

    impl AuthorizationMapping<String> for MockAuthorizationMapping {
        fn get(
            &self,
            _authorization: &AuthorizationHeader,
        ) -> Result<Option<String>, InternalError> {
            Ok(Some("test".to_string()))
        }
    }
}
