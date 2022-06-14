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

use actix_web::dev::*;
use actix_web::{
    http::{
        header::{self, HeaderValue},
        Method as ActixMethod,
    },
    Error as ActixError, HttpMessage, HttpResponse,
};
use futures::{Future, IntoFuture, Poll};

#[cfg(feature = "authorization")]
use crate::rest_api::auth::authorization::{AuthorizationHandler, PermissionMap};
use crate::rest_api::auth::{authorize, identity::IdentityProvider, AuthorizationResult};
use crate::rest_api::ErrorResponse;
#[cfg(feature = "authorization")]
use crate::rest_api::Method;

pub struct AuthorizationMiddleware<S> {
    pub(super) identity_providers: Vec<Box<dyn IdentityProvider>>,
    #[cfg(feature = "authorization")]
    pub(super) authorization_handlers: Vec<Box<dyn AuthorizationHandler>>,
    pub(super) service: S,
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
        let permission_map = match req.app_data::<PermissionMap<Method>>() {
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
            #[cfg(feature = "authorization")]
            &self.authorization_handlers,
        ) {
            AuthorizationResult::Authorized(identity) => {
                debug!("Authenticated user {:?}", identity);
                req.extensions_mut().insert(identity);
            }
            #[cfg(any(
                feature = "authorization",
                feature = "biome-credentials",
                feature = "oauth"
            ))]
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
            #[cfg(feature = "authorization")]
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
