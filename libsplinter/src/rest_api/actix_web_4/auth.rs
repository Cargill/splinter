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

use std::pin::Pin;
use std::str::FromStr;
use std::sync::{Arc, RwLock};

use actix_utils::future::{err, ok, Ready};
use actix_web_4::body::{BoxBody, MessageBody};
use actix_web_4::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web_4::http::header::HeaderMap;
use actix_web_4::HttpResponse;
#[cfg(feature = "cylinder-jwt")]
use cylinder::Verifier;
use futures_0_3::future::{Future, FutureExt};

#[cfg(feature = "biome-credentials")]
use crate::biome::credentials::rest_api::BiomeCredentialsRestResourceProvider;
#[cfg(feature = "oauth")]
use crate::biome::OAuthUserSessionStore;
#[cfg(all(feature = "oauth", feature = "biome-profile"))]
use crate::biome::UserProfileStore;
use crate::rest_api::auth::authorization::Permission;
#[cfg(feature = "authorization")]
use crate::rest_api::auth::authorization::{
    AuthorizationHandler, AuthorizationHandlerResult, PermissionMap,
};
use crate::rest_api::auth::AuthorizationHeader;
#[cfg(feature = "oauth")]
use crate::rest_api::OAuthConfig;
use crate::rest_api::{auth::identity::IdentityProvider, RequestError};
use crate::rest_error::RESTError;

use super::ResourceProvider;

/// Configurations for the various authentication methods supported by the Splinter REST API.
pub enum AuthConfig {
    /// Biome credentials authentication
    #[cfg(feature = "biome-credentials")]
    Biome {
        /// The resource provider that defines the Biome credentials endpoints for the Splinter REST
        /// API
        biome_credentials_resource_provider: BiomeCredentialsRestResourceProvider,
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
        /// The Biome OAuth user session store
        oauth_user_session_store: Box<dyn OAuthUserSessionStore>,
        /// The Biome user profile store
        #[cfg(feature = "biome-profile")]
        user_profile_store: Box<dyn UserProfileStore>,
    },
    /// A custom authentication method
    Custom {
        /// REST API resources that would allow a client to receive some authentication credentials
        resource_provider: Box<dyn ResourceProvider>,
        /// The identity provider that correlates the contents of the `Authorization` header with
        /// an identity for the client
        identity_provider: Box<dyn IdentityProvider>,
    },
}

pub fn require_header(header_key: &str, request: &HeaderMap) -> Result<String, RequestError> {
    let header = request.get(header_key).ok_or_else(|| {
        RequestError::MissingHeader(format!("Header {} not included in Request", header_key))
    })?;
    Ok(header
        .to_str()
        .map_err(|err| RequestError::InvalidHeaderValue(format!("Invalid header value: {}", err)))?
        .to_string())
}

pub fn get_authorization_token(request: &HeaderMap) -> Result<String, RequestError> {
    let auth_header = require_header("Authorization", request)?;
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

#[derive(Default)]
pub(in crate::rest_api) struct AuthTransform {
    identity_providers: Vec<Box<dyn IdentityProvider>>,
    #[cfg(feature = "authorization")]
    authorization_handlers: Vec<Box<dyn AuthorizationHandler>>,
    #[cfg(feature = "authorization")]
    permission_map: Arc<RwLock<PermissionMap>>,
}

impl AuthTransform {
    pub fn new(
        identity_providers: Vec<Box<dyn IdentityProvider>>,
        #[cfg(feature = "authorization")] authorization_handlers: Vec<
            Box<dyn AuthorizationHandler>,
        >,
        #[cfg(feature = "authorization")] permission_map: Arc<RwLock<PermissionMap>>,
    ) -> Self {
        Self {
            identity_providers,
            #[cfg(feature = "authorization")]
            authorization_handlers,
            #[cfg(feature = "authorization")]
            permission_map,
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for AuthTransform
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web_4::error::Error>,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = actix_web_4::error::Error;
    type InitError = ();
    type Transform = AuthService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(AuthService::new(
            service,
            self.identity_providers.clone(),
            self.authorization_handlers.clone(),
            self.permission_map.clone(),
        ))
    }
}

pub(in crate::rest_api) struct AuthService<S> {
    service: S,
    identity_providers: Vec<Box<dyn IdentityProvider>>,
    #[cfg(feature = "authorization")]
    authorization_handlers: Vec<Box<dyn AuthorizationHandler>>,
    #[cfg(feature = "authorization")]
    permission_map: Arc<RwLock<PermissionMap>>,
}

impl<S> AuthService<S> {
    pub fn new(
        service: S,
        identity_providers: Vec<Box<dyn IdentityProvider>>,
        #[cfg(feature = "authorization")] authorization_handlers: Vec<
            Box<dyn AuthorizationHandler>,
        >,
        #[cfg(feature = "authorization")] permission_map: Arc<RwLock<PermissionMap>>,
    ) -> Self {
        Self {
            service,
            identity_providers,
            #[cfg(feature = "authorization")]
            authorization_handlers,
            #[cfg(feature = "authorization")]
            permission_map,
        }
    }
}

impl<S, B> Service<ServiceRequest> for AuthService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web_4::error::Error>,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = actix_web_4::error::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let endpoint = req.path();
        let method = req.method();
        let permission = if let Ok(permission_map) = self.permission_map.read() {
            if let Some(p) = permission_map.get_permission(method, endpoint) {
                *p
            } else {
                return Box::pin(err(
                    RESTError::internal_error("Unknown endpoint", None).into()
                ));
            }
        } else {
            return Box::pin(err(RESTError::internal_error(
                "Could not get permission_map lock",
                None,
            )
            .into()));
        };

        let identity = match get_authorization_token(req.headers()) {
            Ok(auth_token) => match AuthorizationHeader::from_str(&auth_token) {
                Ok(auth_header) => self
                    .identity_providers
                    .iter()
                    .filter_map(|ip| ip.get_identity(&auth_header).ok())
                    .filter(|a| a.is_none())
                    .map(|a| a.unwrap())
                    .next(),
                Err(e) => {
                    return Box::pin(err(RESTError::internal_error(
                        "Could not build auth token from header",
                        Some(Box::new(e)),
                    )
                    .into()));
                }
            },
            Err(e) => return Box::pin(err(e.into())),
        };

        match permission {
            Permission::Check { permission_id, .. } => {
                let identity = if let Some(id) = identity {
                    id
                } else {
                    return Box::pin(ok::<_, actix_web_4::error::Error>(ServiceResponse::new(
                        req.into_parts().0,
                        HttpResponse::Ok().body("Could not find identity"),
                    )));
                };
                let authorized = self
                    .authorization_handlers
                    .iter()
                    .filter_map(|ah| ah.has_permission(&identity, permission_id).ok())
                    .filter_map(|ahr| match ahr {
                        AuthorizationHandlerResult::Allow => Some(true),
                        AuthorizationHandlerResult::Deny => Some(false),
                        AuthorizationHandlerResult::Continue => None,
                    })
                    .next()
                    .unwrap_or(false);
                if authorized {
                    Box::pin(
                        self.service
                            .call(req)
                            .map(|r| r.map(|i| i.map_into_boxed_body())),
                    )
                } else {
                    Box::pin(err(RESTError::NotAuthorized.into()))
                }
            }
            Permission::AllowAuthenticated => match identity {
                Some(_) => Box::pin(
                    self.service
                        .call(req)
                        .map(|r| r.map(|i| i.map_into_boxed_body())),
                ),
                None => Box::pin(err(RESTError::NotAuthorized.into())),
            },
            Permission::AllowUnauthenticated => Box::pin(
                self.service
                    .call(req)
                    .map(|r| r.map(|i| i.map_into_boxed_body())),
            ),
        }
    }

    fn poll_ready(
        &self,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), <Self as Service<ServiceRequest>>::Error>> {
        self.service.poll_ready(context)
    }
}
