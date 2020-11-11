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
use actix_web::{Error as ActixError, HttpMessage, HttpResponse};
use futures::{
    future::{ok, FutureResult},
    Future, IntoFuture, Poll,
};

use crate::error::InternalError;
use crate::rest_api::ErrorResponse;

use super::{
    authorize,
    identity::{Authorization as IdentityAuthorization, GetByAuthorization, IdentityProvider},
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
        dyn Fn(&mut Extensions, &IdentityAuthorization) -> Result<(), InternalError> + Send + Sync,
    >,
}

impl IdentityExtension {
    /// Wrap a GetByAuthorization implementation in an IdentityExtension, to provide the ability to
    /// add a value from the GetByAuthorization trait to the HttpRequest.
    fn new<G, T>(get_by_auth: G) -> Self
    where
        T: 'static,
        G: GetByAuthorization<T> + Send + Sync + 'static,
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
        authorization: &IdentityAuthorization,
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

    /// Add an authorization mapping, provided by a GetByAuthorization implementation.
    pub fn with_authorization_mapping<G, T>(mut self, get_by_auth: G) -> Self
    where
        T: 'static,
        G: GetByAuthorization<T> + Send + Sync + 'static,
    {
        self.identity_extensions
            .push(Arc::new(IdentityExtension::new(get_by_auth)));

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
            AuthorizationResult::NoAuthorizationNecessary => {}
            AuthorizationResult::Unauthorized => {
                return Box::new(
                    req.into_response(HttpResponse::Unauthorized().finish().into_body())
                        .into_future(),
                )
            }
        }

        Box::new(self.service.call(req))
    }
}
