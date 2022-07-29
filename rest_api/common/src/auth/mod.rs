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

mod allow_keys;
mod authorization_handler;
mod authorization_handler_result;
mod authorization_header;
mod authorization_result;
mod bearer_token;
mod identity;
mod maintenance;
#[cfg(feature = "oauth")]
mod oauth;
mod permission;
mod permission_map;
#[cfg(feature = "rbac")]
pub mod rbac;
mod resources;

use log::error;

pub use authorization_handler::AuthorizationHandler;
pub use authorization_handler_result::AuthorizationHandlerResult;
pub use authorization_header::AuthorizationHeader;
pub use authorization_result::AuthorizationResult;
pub use bearer_token::BearerToken;
pub use identity::IdentityProvider;
pub use maintenance::{MaintenanceModeAuthorizationHandler, PostMaintenanceModeQuery};

pub use allow_keys::AllowKeysAuthorizationHandler;
#[cfg(feature = "biome-credentials")]
pub use identity::biome::BiomeUserIdentityProvider;
#[cfg(feature = "cylinder-jwt")]
pub use identity::cylinder::CylinderKeyIdentityProvider;
#[cfg(feature = "oauth")]
pub use identity::oauth::OAuthUserIdentityProvider;
pub use identity::Identity;
#[cfg(feature = "oauth")]
pub use oauth::resources::{
    generate_redirect_query, CallbackQuery, ListOAuthUserResponse, OAuthUserResponse, PagingQuery,
};
#[cfg(all(feature = "authorization", feature = "oauth"))]
pub use oauth::OAUTH_USER_READ_PERMISSION;
pub use permission::Permission;
pub use permission_map::PermissionMap;
#[cfg(feature = "rbac")]
pub use rbac::RoleBasedAuthorizationHandler;
pub use resources::PermissionResponse;

pub const RBAC_READ_PERMISSION: Permission = Permission::Check {
    permission_id: "authorization.rbac.read",
    permission_display_name: "RBAC read",
    permission_description: "Allows the client to read roles, identities, and role assignments",
};

pub const RBAC_WRITE_PERMISSION: Permission = Permission::Check {
    permission_id: "authorization.rbac.write",
    permission_display_name: "RBAC write",
    permission_description: "Allows the client to modify roles, identities, and role assignments",
};

/// Uses the given identity providers to check authorization for the request. This function is
/// backend-agnostic and intended as a helper for the backend REST API implementations.
///
/// # Arguments
///
/// * `method` - The HTTP method used for the request
/// * `endpoint` - The endpoint that is being requested. Example: "/endpoint/path"
/// * `auth_header` - The value of the Authorization HTTP header for the request
/// * `identity_providers` - The identity providers that will be used to check the client's identity
/// * `authorization_handlers` - The authorization handlers that will be used to check the client's
///   permissions
pub fn authorize<M: PartialEq + Clone>(
    #[cfg(feature = "authorization")] method: &M,
    #[cfg(any(
        feature = "authorization",
        feature = "biome-credentials",
        feature = "oauth"
    ))]
    endpoint: &str,
    #[cfg(not(any(
        feature = "authorization",
        feature = "biome-credentials",
        feature = "oauth"
    )))]
    _endpoint: &str,
    auth_header: Option<&str>,
    #[cfg(feature = "authorization")] permission_map: &PermissionMap<M>,
    identity_providers: &[Box<dyn IdentityProvider>],
    #[cfg(feature = "authorization")] authorization_handlers: &[Box<dyn AuthorizationHandler>],
) -> AuthorizationResult {
    #[cfg(feature = "authorization")]
    {
        // Get the permission that applies to this request
        let permission = match permission_map.get_permission(method, endpoint) {
            Some(perm) => perm,
            None => return AuthorizationResult::UnknownEndpoint,
        };

        match *permission {
            Permission::AllowUnauthenticated => AuthorizationResult::NoAuthorizationNecessary,
            Permission::AllowAuthenticated => match get_identity(auth_header, identity_providers) {
                Some(identity) => AuthorizationResult::Authorized(identity),
                None => AuthorizationResult::Unauthorized,
            },
            Permission::Check { permission_id, .. } => {
                match get_identity(auth_header, identity_providers) {
                    Some(identity) => {
                        for handler in authorization_handlers {
                            match handler.has_permission(&identity, permission_id) {
                                Ok(AuthorizationHandlerResult::Allow) => {
                                    return AuthorizationResult::Authorized(identity)
                                }
                                Ok(AuthorizationHandlerResult::Deny) => {
                                    return AuthorizationResult::Unauthorized
                                }
                                Ok(AuthorizationHandlerResult::Continue) => {}
                                Err(err) => error!("{}", err),
                            }
                        }
                        // No handler allowed the request, so deny by default
                        AuthorizationResult::Unauthorized
                    }
                    None => AuthorizationResult::Unauthorized,
                }
            }
        }
    }
    #[cfg(not(feature = "authorization"))]
    {
        #[cfg(any(feature = "biome-credentials", feature = "oauth"))]
        {
            // Authorization isn't necessary when using one of the authorization endpoints
            let mut is_auth_endpoint = false;
            #[cfg(feature = "biome-credentials")]
            if endpoint == "/biome/register"
                || endpoint == "/biome/login"
                || endpoint == "/biome/token"
            {
                is_auth_endpoint = true;
            }
            #[cfg(feature = "oauth")]
            if endpoint == "/oauth/login" || endpoint == "/oauth/callback" {
                is_auth_endpoint = true;
            }
            if is_auth_endpoint {
                return AuthorizationResult::NoAuthorizationNecessary;
            }
        }

        match get_identity(auth_header, identity_providers) {
            Some(identity) => AuthorizationResult::Authorized(identity),
            None => AuthorizationResult::Unauthorized,
        }
    }
}

fn get_identity(
    auth_header: Option<&str>,
    identity_providers: &[Box<dyn IdentityProvider>],
) -> Option<Identity> {
    let authorization = auth_header?.parse().ok()?;
    identity_providers.iter().find_map(|provider| {
        provider.get_identity(&authorization).unwrap_or_else(|err| {
            error!("{}", err);
            None
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::auth::permission_map::Method;
    use splinter::error::InternalError;

    /// Verifies that the `authorize` function returns `AuthorizationResult::Unauthorized` when no
    /// identity providers are specified. Without any identity providers, there's no way to properly
    /// authorize.
    #[test]
    fn authorize_no_identity_providers() {
        #[cfg(feature = "authorization")]
        let permission_map = {
            let mut map = PermissionMap::new();
            map.add_permission(
                Method::Get,
                "/test/endpoint",
                Permission::AllowAuthenticated,
            );
            map
        };

        assert!(matches!(
            authorize::<Method>(
                #[cfg(feature = "authorization")]
                &Method::Get,
                "/test/endpoint",
                Some("auth"),
                #[cfg(feature = "authorization")]
                &permission_map,
                &[],
                #[cfg(feature = "authorization")]
                &[Box::new(AlwaysAllowAuthorizationHandler)],
            ),
            AuthorizationResult::Unauthorized
        ));
    }

    /// Verifies that the `authorize` function returns `AuthorizationResult::Unauthorized` when no
    /// identity provider returns an identity for the auth header.
    #[test]
    fn authorize_no_matching_identity_provider() {
        #[cfg(feature = "authorization")]
        let permission_map = {
            let mut map = PermissionMap::new();
            map.add_permission(
                Method::Get,
                "/test/endpoint",
                Permission::AllowAuthenticated,
            );
            map
        };

        assert!(matches!(
            authorize::<Method>(
                #[cfg(feature = "authorization")]
                &Method::Get,
                "/test/endpoint",
                Some("auth"),
                #[cfg(feature = "authorization")]
                &permission_map,
                &[Box::new(AlwaysRejectIdentityProvider)],
                #[cfg(feature = "authorization")]
                &[Box::new(AlwaysAllowAuthorizationHandler)],
            ),
            AuthorizationResult::Unauthorized
        ));
    }

    /// Verifies that the `authorize` function returns `AuthorizationResult::Unauthorized` when no
    /// auth header is provided. By using an identity provider that always successfully gets an
    /// identity, we verify that the failure is because of the missing header value.
    #[test]
    fn authorize_no_header_provided() {
        #[cfg(feature = "authorization")]
        let permission_map = {
            let mut map = PermissionMap::new();
            map.add_permission(
                Method::Get,
                "/test/endpoint",
                Permission::AllowAuthenticated,
            );
            map
        };

        assert!(matches!(
            authorize::<Method>(
                #[cfg(feature = "authorization")]
                &Method::Get,
                "/test/endpoint",
                None,
                #[cfg(feature = "authorization")]
                &permission_map,
                &[Box::new(AlwaysAcceptIdentityProvider)],
                #[cfg(feature = "authorization")]
                &[Box::new(AlwaysAllowAuthorizationHandler)],
            ),
            AuthorizationResult::Unauthorized
        ));
    }

    /// Verifies that the `authorize` function returns `AuthorizationResult::UnknownEndpoint` when
    /// a permission cannot be found for the request. This is accomplished by passing in an empty
    /// permission map.
    #[cfg(feature = "authorization")]
    #[test]
    fn authorize_unknown_endpoint() {
        assert!(matches!(
            authorize(
                &Method::Get,
                "/test/endpoint",
                None,
                &Default::default(),
                &[Box::new(AlwaysAcceptIdentityProvider)],
                &[Box::new(AlwaysAllowAuthorizationHandler)],
            ),
            AuthorizationResult::UnknownEndpoint
        ));
    }

    /// Verifies that the `authorization` function returns
    /// `AuthorizationResult::NoAuthorizationNecessary` when the requested endpoint does not require
    /// authorization, whether the header is set or not. By using an identity provider that always
    /// returns `None`, we verify that authorization is being ignored.
    #[test]
    fn authorize_no_authorization_necessary() {
        #[cfg(feature = "authorization")]
        {
            let mut permission_map = PermissionMap::new();
            permission_map.add_permission(
                Method::Get,
                "/test/endpoint",
                Permission::AllowUnauthenticated,
            );

            // Verify with header not set
            assert!(matches!(
                authorize(
                    &Method::Get,
                    "/test/endpoint",
                    None,
                    &permission_map,
                    &[Box::new(AlwaysRejectIdentityProvider)],
                    &[Box::new(AlwaysAllowAuthorizationHandler)],
                ),
                AuthorizationResult::NoAuthorizationNecessary
            ));

            // Verify with header set
            assert!(matches!(
                authorize(
                    &Method::Get,
                    "/test/endpoint",
                    Some("auth"),
                    &permission_map,
                    &[Box::new(AlwaysRejectIdentityProvider)],
                    &[Box::new(AlwaysAllowAuthorizationHandler)],
                ),
                AuthorizationResult::NoAuthorizationNecessary
            ));
        }

        #[cfg(not(feature = "authorization"))]
        {
            // Verify with header not set
            #[cfg(feature = "biome-credentials")]
            {
                assert!(matches!(
                    authorize(
                        "/biome/register",
                        None,
                        &[Box::new(AlwaysRejectIdentityProvider)]
                    ),
                    AuthorizationResult::NoAuthorizationNecessary
                ));
                assert!(matches!(
                    authorize(
                        "/biome/login",
                        None,
                        &[Box::new(AlwaysRejectIdentityProvider)]
                    ),
                    AuthorizationResult::NoAuthorizationNecessary
                ));
                assert!(matches!(
                    authorize(
                        "/biome/token",
                        None,
                        &[Box::new(AlwaysRejectIdentityProvider)]
                    ),
                    AuthorizationResult::NoAuthorizationNecessary
                ));
            }
            #[cfg(feature = "oauth")]
            {
                assert!(matches!(
                    authorize(
                        "/oauth/login",
                        None,
                        &[Box::new(AlwaysRejectIdentityProvider)]
                    ),
                    AuthorizationResult::NoAuthorizationNecessary
                ));
                assert!(matches!(
                    authorize(
                        "/oauth/callback",
                        None,
                        &[Box::new(AlwaysRejectIdentityProvider)]
                    ),
                    AuthorizationResult::NoAuthorizationNecessary
                ));
            }

            // Verify with header set
            #[cfg(feature = "biome-credentials")]
            {
                assert!(matches!(
                    authorize(
                        "/biome/register",
                        Some("auth"),
                        &[Box::new(AlwaysRejectIdentityProvider)]
                    ),
                    AuthorizationResult::NoAuthorizationNecessary
                ));
                assert!(matches!(
                    authorize(
                        "/biome/login",
                        Some("auth"),
                        &[Box::new(AlwaysRejectIdentityProvider)]
                    ),
                    AuthorizationResult::NoAuthorizationNecessary
                ));
                assert!(matches!(
                    authorize(
                        "/biome/token",
                        Some("auth"),
                        &[Box::new(AlwaysRejectIdentityProvider)]
                    ),
                    AuthorizationResult::NoAuthorizationNecessary
                ));
            }
            #[cfg(feature = "oauth")]
            {
                assert!(matches!(
                    authorize(
                        "/oauth/login",
                        Some("auth"),
                        &[Box::new(AlwaysRejectIdentityProvider)]
                    ),
                    AuthorizationResult::NoAuthorizationNecessary
                ));
                assert!(matches!(
                    authorize(
                        "/oauth/callback",
                        Some("auth"),
                        &[Box::new(AlwaysRejectIdentityProvider)]
                    ),
                    AuthorizationResult::NoAuthorizationNecessary
                ));
            }
        }
    }

    /// Verifies the simple case where `authorize` is called with a single identity provider that
    /// successfully gets the client's identity. This test uses an endpoint with
    /// `Permission::AllowAuthenticated` to ignore authorization handlers which are covered by other
    /// tests.
    #[test]
    fn authorize_successful_one_identity_provider() {
        let expected_auth = "auth".parse().unwrap();
        let expected_identity = AlwaysAcceptIdentityProvider
            .get_identity(&expected_auth)
            .unwrap()
            .unwrap();

        #[cfg(feature = "authorization")]
        let permission_map = {
            let mut map = PermissionMap::new();
            map.add_permission(
                Method::Get,
                "/test/endpoint",
                Permission::AllowAuthenticated,
            );
            map
        };

        assert!(matches!(
            authorize::<Method>(
                #[cfg(feature = "authorization")]
                &Method::Get,
                "/test/endpoint",
                Some("auth"),
                #[cfg(feature = "authorization")]
                &permission_map,
                &[Box::new(AlwaysAcceptIdentityProvider)],
                #[cfg(feature = "authorization")]
                &[Box::new(AlwaysAllowAuthorizationHandler)],
            ),
            AuthorizationResult::Authorized(identity) if identity == expected_identity
        ));
    }

    /// Verifies the case where `authorize` is called with multiple identity providers and only one
    /// successfully gets the client's identity. This test uses an endpoint with
    /// `Permission::AllowAuthenticated` to ignore authorization handlers which are covered by other
    /// tests.
    #[test]
    fn authorize_successful_multiple_identity_providers() {
        let expected_auth = "auth".parse().unwrap();
        let expected_identity = AlwaysAcceptIdentityProvider
            .get_identity(&expected_auth)
            .unwrap()
            .unwrap();

        #[cfg(feature = "authorization")]
        let permission_map = {
            let mut map = PermissionMap::new();
            map.add_permission(
                Method::Get,
                "/test/endpoint",
                Permission::AllowAuthenticated,
            );
            map
        };

        assert!(matches!(
            authorize::<Method>(
                #[cfg(feature = "authorization")]
                &Method::Get,
                "/test/endpoint",
                Some("auth"),
                #[cfg(feature = "authorization")]
                &permission_map,
                &[
                    Box::new(AlwaysRejectIdentityProvider),
                    Box::new(AlwaysAcceptIdentityProvider),
                    Box::new(AlwaysRejectIdentityProvider),
                ],
                #[cfg(feature = "authorization")]
                &[Box::new(AlwaysAllowAuthorizationHandler)],
            ),
            AuthorizationResult::Authorized(identity) if identity == expected_identity
        ));
    }

    /// Verifies the case where `authorize` is called with multiple identity provider and one errors
    /// before another successfully gets the client's identity. This test uses an endpoint with
    /// `Permission::AllowAuthenticated` to ignore authorization handlers which are covered by other
    /// tests.
    #[test]
    fn authorize_successful_after_identity_provider_error() {
        let expected_auth = "auth".parse().unwrap();
        let expected_identity = AlwaysAcceptIdentityProvider
            .get_identity(&expected_auth)
            .unwrap()
            .unwrap();

        #[cfg(feature = "authorization")]
        let permission_map = {
            let mut map = PermissionMap::new();
            map.add_permission(
                Method::Get,
                "/test/endpoint",
                Permission::AllowAuthenticated,
            );
            map
        };

        assert!(matches!(
            authorize::<Method>(
                #[cfg(feature = "authorization")]
                &Method::Get,
                "/test/endpoint",
                Some("auth"),
                #[cfg(feature = "authorization")]
                &permission_map,
                &[
                    Box::new(AlwaysErrIdentityProvider),
                    Box::new(AlwaysAcceptIdentityProvider),
                ],
                #[cfg(feature = "authorization")]
                &[Box::new(AlwaysAllowAuthorizationHandler)],
            ),
            AuthorizationResult::Authorized(identity) if identity == expected_identity
        ));
    }

    /// Verifies that the `authorize` function returns `AuthorizationResult::Unauthorized` when no
    /// authorization handlers are specified, since this function should deny by default.
    #[cfg(feature = "authorization")]
    #[test]
    fn authorize_no_authorization_handlers() {
        let permission_map = {
            let mut map = PermissionMap::new();
            map.add_permission(
                Method::Get,
                "/test/endpoint",
                Permission::Check {
                    permission_id: "permission",
                    permission_display_name: "",
                    permission_description: "",
                },
            );
            map
        };

        assert!(matches!(
            authorize(
                &Method::Get,
                "/test/endpoint",
                Some("auth"),
                &permission_map,
                &[Box::new(AlwaysAcceptIdentityProvider)],
                &[],
            ),
            AuthorizationResult::Unauthorized
        ));
    }

    /// Verifies that the `authorize` function returns `AuthorizationResult::Unauthorized` when no
    /// authorization handlers returns `Allow` or `Deny`. since this function should deny by
    /// default (must get an explicit `Allow` from an authorization handler).
    #[cfg(feature = "authorization")]
    #[test]
    fn authorize_no_allowing_authorization_handler() {
        let permission_map = {
            let mut map = PermissionMap::new();
            map.add_permission(
                Method::Get,
                "/test/endpoint",
                Permission::Check {
                    permission_id: "permission",
                    permission_display_name: "",
                    permission_description: "",
                },
            );
            map
        };

        assert!(matches!(
            authorize(
                &Method::Get,
                "/test/endpoint",
                Some("auth"),
                &permission_map,
                &[Box::new(AlwaysAcceptIdentityProvider)],
                &[Box::new(AlwaysContinueAuthorizationHandler)],
            ),
            AuthorizationResult::Unauthorized
        ));
    }

    /// Verifies that the `authorize` function returns `AuthorizationResult::Unauthorized` when an
    /// authorization handler returns `Deny`, even if an auth handler after it returns `Allow`.
    #[cfg(feature = "authorization")]
    #[test]
    fn authorize_deny_before_allowing_authorization_handler() {
        let permission_map = {
            let mut map = PermissionMap::new();
            map.add_permission(
                Method::Get,
                "/test/endpoint",
                Permission::Check {
                    permission_id: "permission",
                    permission_display_name: "",
                    permission_description: "",
                },
            );
            map
        };

        assert!(matches!(
            authorize(
                &Method::Get,
                "/test/endpoint",
                Some("auth"),
                &permission_map,
                &[Box::new(AlwaysAcceptIdentityProvider)],
                &[
                    Box::new(AlwaysDenyAuthorizationHandler),
                    Box::new(AlwaysAllowAuthorizationHandler),
                ],
            ),
            AuthorizationResult::Unauthorized
        ));
    }

    /// Verifies that the `authorize` function returns `AuthorizationResult::Authorized(identity)`
    /// when an authorization handler returns `Allow`, even if an auth handler after it returns
    /// `Deny`.
    #[cfg(feature = "authorization")]
    #[test]
    fn authorize_allow_before_denying_authorization_handler() {
        let expected_auth = "auth".parse().unwrap();
        let expected_identity = AlwaysAcceptIdentityProvider
            .get_identity(&expected_auth)
            .unwrap()
            .unwrap();

        let permission_map = {
            let mut map = PermissionMap::new();
            map.add_permission(
                Method::Get,
                "/test/endpoint",
                Permission::Check {
                    permission_id: "permission",
                    permission_display_name: "",
                    permission_description: "",
                },
            );
            map
        };

        assert!(matches!(
            authorize(
                &Method::Get,
                "/test/endpoint",
                Some("auth"),
                &permission_map,
                &[Box::new(AlwaysAcceptIdentityProvider)],
                &[
                    Box::new(AlwaysAllowAuthorizationHandler),
                    Box::new(AlwaysDenyAuthorizationHandler),
                ],
            ),
            AuthorizationResult::Authorized(identity) if identity == expected_identity
        ));
    }

    /// Verifies that the `authorize` function returns `AuthorizationResult::Authorized(identity)`
    /// when an authorization handler returns `Allow` after another authorization handler returns
    /// `Continue`.
    #[cfg(feature = "authorization")]
    #[test]
    fn authorize_allow_after_continuing_authorization_handler() {
        let expected_auth = "auth".parse().unwrap();
        let expected_identity = AlwaysAcceptIdentityProvider
            .get_identity(&expected_auth)
            .unwrap()
            .unwrap();

        let permission_map = {
            let mut map = PermissionMap::new();
            map.add_permission(
                Method::Get,
                "/test/endpoint",
                Permission::Check {
                    permission_id: "permission",
                    permission_display_name: "",
                    permission_description: "",
                },
            );
            map
        };

        assert!(matches!(
            authorize(
                &Method::Get,
                "/test/endpoint",
                Some("auth"),
                &permission_map,
                &[Box::new(AlwaysAcceptIdentityProvider)],
                &[
                    Box::new(AlwaysContinueAuthorizationHandler),
                    Box::new(AlwaysAllowAuthorizationHandler),
                ],
            ),
            AuthorizationResult::Authorized(identity) if identity == expected_identity
        ));
    }

    /// Verifies that the `authorize` function returns `AuthorizationResult::Authorized(identity)`
    /// when an authorization handler returns `Allow` after another authorization handler returns
    /// `Err(_)`.
    #[cfg(feature = "authorization")]
    #[test]
    fn authorize_allow_after_err_authorization_handler() {
        let expected_auth = "auth".parse().unwrap();
        let expected_identity = AlwaysAcceptIdentityProvider
            .get_identity(&expected_auth)
            .unwrap()
            .unwrap();

        let permission_map = {
            let mut map = PermissionMap::new();
            map.add_permission(
                Method::Get,
                "/test/endpoint",
                Permission::Check {
                    permission_id: "permission",
                    permission_display_name: "",
                    permission_description: "",
                },
            );
            map
        };

        assert!(matches!(
            authorize(
                &Method::Get,
                "/test/endpoint",
                Some("auth"),
                &permission_map,
                &[Box::new(AlwaysAcceptIdentityProvider)],
                &[
                    Box::new(AlwaysErrAuthorizationHandler),
                    Box::new(AlwaysAllowAuthorizationHandler),
                ],
            ),
            AuthorizationResult::Authorized(identity) if identity == expected_identity
        ));
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

    /// An identity provider that always returns `Ok(None)`
    #[derive(Clone)]
    struct AlwaysRejectIdentityProvider;

    impl IdentityProvider for AlwaysRejectIdentityProvider {
        fn get_identity(
            &self,
            _authorization: &AuthorizationHeader,
        ) -> Result<Option<Identity>, InternalError> {
            Ok(None)
        }

        fn clone_box(&self) -> Box<dyn IdentityProvider> {
            Box::new(self.clone())
        }
    }

    /// An identity provider that always returns `Err(_)`
    #[derive(Clone)]
    struct AlwaysErrIdentityProvider;

    impl IdentityProvider for AlwaysErrIdentityProvider {
        fn get_identity(
            &self,
            _authorization: &AuthorizationHeader,
        ) -> Result<Option<Identity>, InternalError> {
            Err(InternalError::with_message("failed".into()))
        }

        fn clone_box(&self) -> Box<dyn IdentityProvider> {
            Box::new(self.clone())
        }
    }

    /// An authorization handler that always returns `Ok(AuthorizationHandlerResult::Allow)`
    #[cfg(feature = "authorization")]
    #[derive(Clone)]
    struct AlwaysAllowAuthorizationHandler;

    #[cfg(feature = "authorization")]
    impl AuthorizationHandler for AlwaysAllowAuthorizationHandler {
        fn has_permission(
            &self,
            _identity: &Identity,
            _permission_id: &str,
        ) -> Result<AuthorizationHandlerResult, InternalError> {
            Ok(AuthorizationHandlerResult::Allow)
        }

        fn clone_box(&self) -> Box<dyn AuthorizationHandler> {
            Box::new(self.clone())
        }
    }

    /// An authorization handler that always returns `Ok(AuthorizationHandlerResult::Deny)`
    #[cfg(feature = "authorization")]
    #[derive(Clone)]
    struct AlwaysDenyAuthorizationHandler;

    #[cfg(feature = "authorization")]
    impl AuthorizationHandler for AlwaysDenyAuthorizationHandler {
        fn has_permission(
            &self,
            _identity: &Identity,
            _permission_id: &str,
        ) -> Result<AuthorizationHandlerResult, InternalError> {
            Ok(AuthorizationHandlerResult::Deny)
        }

        fn clone_box(&self) -> Box<dyn AuthorizationHandler> {
            Box::new(self.clone())
        }
    }

    /// An authorization handler that always returns `Ok(AuthorizationHandlerResult::Continue)`
    #[cfg(feature = "authorization")]
    #[derive(Clone)]
    struct AlwaysContinueAuthorizationHandler;

    #[cfg(feature = "authorization")]
    impl AuthorizationHandler for AlwaysContinueAuthorizationHandler {
        fn has_permission(
            &self,
            _identity: &Identity,
            _permission_id: &str,
        ) -> Result<AuthorizationHandlerResult, InternalError> {
            Ok(AuthorizationHandlerResult::Continue)
        }

        fn clone_box(&self) -> Box<dyn AuthorizationHandler> {
            Box::new(self.clone())
        }
    }

    /// An authorization handler that always returns `Err(_)`
    #[cfg(feature = "authorization")]
    #[derive(Clone)]
    struct AlwaysErrAuthorizationHandler;

    #[cfg(feature = "authorization")]
    impl AuthorizationHandler for AlwaysErrAuthorizationHandler {
        fn has_permission(
            &self,
            _identity: &Identity,
            _permission_id: &str,
        ) -> Result<AuthorizationHandlerResult, InternalError> {
            Err(InternalError::with_message("failed".into()))
        }

        fn clone_box(&self) -> Box<dyn AuthorizationHandler> {
            Box::new(self.clone())
        }
    }
}
