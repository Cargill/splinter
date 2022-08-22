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

//! Authentication and authorization for Splinter

#[cfg(feature = "rest-api-actix-web-1")]
pub(crate) mod actix;
#[cfg(feature = "authorization")]
pub mod authorization;
pub mod identity;

use std::str::FromStr;

use crate::error::InvalidArgumentError;

#[cfg(feature = "authorization")]
use super::Method;

#[cfg(feature = "authorization")]
use authorization::{AuthorizationHandler, AuthorizationHandlerResult, Permission, PermissionMap};
#[cfg(feature = "rest-api-actix-web-1")]
use identity::{Identity, IdentityProvider};

/// The possible outcomes of attempting to authorize a client
#[cfg(feature = "rest-api-actix-web-1")]
enum AuthorizationResult {
    /// The client was authorized to the given identity based on the authorization header
    Authorized(Identity),
    /// The requested endpoint does not require authorization
    #[cfg(any(
        feature = "authorization",
        feature = "biome-credentials",
        feature = "oauth"
    ))]
    NoAuthorizationNecessary,
    /// The authorization header is empty or invalid
    Unauthorized,
    /// The request endpoint is not defined
    #[cfg(feature = "authorization")]
    UnknownEndpoint,
}

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
#[cfg(feature = "rest-api-actix-web-1")]
fn authorize(
    #[cfg(feature = "authorization")] method: &Method,
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
    #[cfg(feature = "authorization")] permission_map: &PermissionMap,
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

#[cfg(feature = "rest-api-actix-web-1")]
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

/// A parsed authorization header
#[derive(PartialEq, Eq)]
pub enum AuthorizationHeader {
    Bearer(BearerToken),
    Custom(String),
}

/// Parses an authorization string. This implementation will attempt to parse the string in the
/// format "<scheme> <value>" to a known scheme. If the string does not match this format or the
/// scheme is unknown, the `AuthorizationHeader::Custom` variant will be returned with the whole
/// authorization string.
impl FromStr for AuthorizationHeader {
    type Err = InvalidArgumentError;

    fn from_str(str: &str) -> Result<Self, Self::Err> {
        let mut parts = str.splitn(2, ' ');
        match (parts.next(), parts.next()) {
            (Some(auth_scheme), Some(value)) => match auth_scheme {
                "Bearer" => Ok(AuthorizationHeader::Bearer(value.parse()?)),
                _ => Ok(AuthorizationHeader::Custom(str.to_string())),
            },
            (Some(_), None) => Ok(AuthorizationHeader::Custom(str.to_string())),
            _ => unreachable!(), // splitn always returns at least one item
        }
    }
}

/// A bearer token of a specific type
#[derive(PartialEq, Eq)]
pub enum BearerToken {
    #[cfg(feature = "biome-credentials")]
    /// Contains a Biome JWT
    Biome(String),
    /// Contains a custom token, which is any bearer token that does not match one of the other
    /// variants of this enum
    Custom(String),
    #[cfg(feature = "cylinder-jwt")]
    /// Contains a Cylinder JWT
    Cylinder(String),
    #[cfg(feature = "oauth")]
    /// Contains an OAuth2 token
    OAuth2(String),
}

/// Parses a bearer token string. This implementation will attempt to parse the token in the format
/// "<type>:<value>" to a know type. If the token does not match this format or the type is unknown,
/// the `BearerToken::Custom` variant will be returned with the whole token value.
impl FromStr for BearerToken {
    type Err = InvalidArgumentError;

    fn from_str(str: &str) -> Result<Self, Self::Err> {
        let mut parts = str.splitn(2, ':');
        match (parts.next(), parts.next()) {
            // Allowing lint in case none of `biome-credentials`, `cylinder-jwt`, or `oauth` are
            // used
            #[allow(unused_variables, clippy::match_single_binding)]
            (Some(token_type), Some(token)) => match token_type {
                #[cfg(feature = "biome-credentials")]
                "Biome" => Ok(BearerToken::Biome(token.to_string())),
                #[cfg(feature = "cylinder-jwt")]
                "Cylinder" => Ok(BearerToken::Cylinder(token.to_string())),
                #[cfg(feature = "oauth")]
                "OAuth2" => Ok(BearerToken::OAuth2(token.to_string())),
                _ => Ok(BearerToken::Custom(str.to_string())),
            },
            (Some(_), None) => Ok(BearerToken::Custom(str.to_string())),
            _ => unreachable!(), // splitn always returns at least one item
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::error::InternalError;

    /// Verfifies that the `AuthorizationHeader` enum is correctly parsed from strings
    #[test]
    fn parse_authorization_header() {
        assert!(matches!(
            "Bearer token".parse(),
            Ok(AuthorizationHeader::Bearer(token)) if token == "token".parse().unwrap()
        ));

        assert!(matches!(
            "Unknown token".parse(),
            Ok(AuthorizationHeader::Custom(auth_str)) if auth_str == "Unknown token"
        ));

        assert!(matches!(
            "test".parse(),
            Ok(AuthorizationHeader::Custom(auth_str)) if auth_str == "test"
        ));
    }

    /// Verfifies that the `BearerToken` enum is correctly parsed from strings
    #[test]
    fn parse_bearer_token() {
        #[cfg(feature = "biome-credentials")]
        assert!(matches!(
            "Biome:test".parse(),
            Ok(BearerToken::Biome(token)) if token == "test"
        ));

        #[cfg(feature = "cylinder-jwt")]
        assert!(matches!(
            "Cylinder:test".parse(),
            Ok(BearerToken::Cylinder(token)) if token == "test"
        ));

        #[cfg(feature = "oauth")]
        assert!(matches!(
            "OAuth2:test".parse(),
            Ok(BearerToken::OAuth2(token)) if token == "test"
        ));

        assert!(matches!(
            "Unknown:test".parse(),
            Ok(BearerToken::Custom(token)) if token == "Unknown:test"
        ));

        assert!(matches!(
            "test".parse(),
            Ok(BearerToken::Custom(token)) if token == "test"
        ));
    }

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
            authorize(
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
            authorize(
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
            authorize(
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
            authorize(
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

    /// Verifies the case where `authorize` is called with multile identity providers and only one
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
            authorize(
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
            authorize(
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
