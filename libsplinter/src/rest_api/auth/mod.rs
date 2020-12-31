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

//! Authentication and authorization for Splinter

#[cfg(feature = "rest-api-actix")]
pub(crate) mod actix;
pub mod identity;
#[cfg(feature = "authorization")]
mod permission_map;
#[cfg(feature = "authorization")]
pub mod roles;

use std::str::FromStr;

use crate::error::InvalidArgumentError;

#[cfg(feature = "authorization")]
use super::Method;

use identity::{Identity, IdentityProvider};

#[cfg(feature = "authorization")]
#[derive(Clone, Debug, PartialEq)]
pub enum Permission {
    /// Check that the authenticated client has the specified permission.
    Check(&'static str),
    /// Allow any request that has been authenticated (the client's identity has been determined).
    /// This may be used by endpoints that need to know the client's identity but do not require a
    /// special permission to be checked (the Biome key management and OAuth logout routes are an
    /// example of this).
    AllowAuthenticated,
    /// Allow any request without checking for authorization.
    AllowUnauthenticated,
}

/// The possible outcomes of attempting to authorize a client
enum AuthorizationResult {
    /// The client was authorized to the given identity based on the authorization header
    Authorized(Identity),
    /// The requested endpoint does not require authorization
    NoAuthorizationNecessary,
    /// The authorization header is empty or invalid
    Unauthorized,
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
fn authorize(
    #[cfg(feature = "authorization")] _method: &Method,
    endpoint: &str,
    auth_header: Option<&str>,
    identity_providers: &[Box<dyn IdentityProvider>],
) -> AuthorizationResult {
    // Authorization isn't necessary when using one of the authorization endpoints
    let mut is_auth_endpoint = false;
    #[cfg(feature = "biome-credentials")]
    if endpoint == "/biome/register" || endpoint == "/biome/login" || endpoint == "/biome/token" {
        is_auth_endpoint = true;
    }
    #[cfg(feature = "oauth")]
    if endpoint == "/oauth/login" || endpoint == "/oauth/callback" {
        is_auth_endpoint = true;
    }
    if is_auth_endpoint {
        return AuthorizationResult::NoAuthorizationNecessary;
    }

    // Parse the auth header
    let auth_str = match auth_header {
        Some(auth_str) => auth_str,
        None => return AuthorizationResult::Unauthorized,
    };
    let authorization = match auth_str.parse() {
        Ok(auth) => auth,
        Err(_) => return AuthorizationResult::Unauthorized,
    };

    // Attempt to get the client's identity
    for provider in identity_providers {
        match provider.get_identity(&authorization) {
            Ok(Some(identity)) => return AuthorizationResult::Authorized(identity),
            Ok(None) => {}
            Err(err) => error!("{}", err),
        }
    }

    // No identity provider could resolve the authorization to an identity
    AuthorizationResult::Unauthorized
}

/// A parsed authorization header
#[derive(PartialEq)]
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
#[derive(PartialEq)]
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
        assert!(matches!(
            authorize(
                #[cfg(feature = "authorization")]
                &Method::Get,
                "/test/endpoint",
                Some("auth"),
                &[]
            ),
            AuthorizationResult::Unauthorized
        ));
    }

    /// Verifies that the `authorize` function returns `AuthorizationResult::Unauthorized` when no
    /// identity provider returns an identity for the auth header.
    #[test]
    fn authorize_no_matching_identity_provider() {
        assert!(matches!(
            authorize(
                #[cfg(feature = "authorization")]
                &Method::Get,
                "/test/endpoint",
                Some("auth"),
                &[Box::new(AlwaysRejectIdentityProvider)]
            ),
            AuthorizationResult::Unauthorized
        ));
    }

    /// Verifies that the `authorize` function returns `AuthorizationResult::Unauthorized` when no
    /// auth header is provided. By using an identity provider that always successfully gets an
    /// identity, we verify that the failure is because of the missing header value.
    #[test]
    fn authorize_no_header_provided() {
        assert!(matches!(
            authorize(
                #[cfg(feature = "authorization")]
                &Method::Get,
                "/test/endpoint",
                None,
                &[Box::new(AlwaysAcceptIdentityProvider)]
            ),
            AuthorizationResult::Unauthorized
        ));
    }

    /// Verifies that the `authorization` function returns
    /// `AuthorizationResult::NoAuthorizationNecessary`when the requested endpoint does not require
    /// authorization, whether the header is set or not. By using an identity provider that always
    /// returns `None`, we verify that authorization is being ignored.
    #[test]
    fn authorize_no_authorization_necessary() {
        // Verify with header not set
        #[cfg(feature = "biome-credentials")]
        {
            assert!(matches!(
                authorize(
                    #[cfg(feature = "authorization")]
                    &Method::Get,
                    "/biome/register",
                    None,
                    &[Box::new(AlwaysRejectIdentityProvider)]
                ),
                AuthorizationResult::NoAuthorizationNecessary
            ));
            assert!(matches!(
                authorize(
                    #[cfg(feature = "authorization")]
                    &Method::Get,
                    "/biome/login",
                    None,
                    &[Box::new(AlwaysRejectIdentityProvider)]
                ),
                AuthorizationResult::NoAuthorizationNecessary
            ));
            assert!(matches!(
                authorize(
                    #[cfg(feature = "authorization")]
                    &Method::Get,
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
                    #[cfg(feature = "authorization")]
                    &Method::Get,
                    "/oauth/login",
                    None,
                    &[Box::new(AlwaysRejectIdentityProvider)]
                ),
                AuthorizationResult::NoAuthorizationNecessary
            ));
            assert!(matches!(
                authorize(
                    #[cfg(feature = "authorization")]
                    &Method::Get,
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
                    #[cfg(feature = "authorization")]
                    &Method::Get,
                    "/biome/register",
                    Some("auth"),
                    &[Box::new(AlwaysRejectIdentityProvider)]
                ),
                AuthorizationResult::NoAuthorizationNecessary
            ));
            assert!(matches!(
                authorize(
                    #[cfg(feature = "authorization")]
                    &Method::Get,
                    "/biome/login",
                    Some("auth"),
                    &[Box::new(AlwaysRejectIdentityProvider)]
                ),
                AuthorizationResult::NoAuthorizationNecessary
            ));
            assert!(matches!(
                authorize(
                    #[cfg(feature = "authorization")]
                    &Method::Get,
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
                    #[cfg(feature = "authorization")]
                    &Method::Get,
                    "/oauth/login",
                    Some("auth"),
                    &[Box::new(AlwaysRejectIdentityProvider)]
                ),
                AuthorizationResult::NoAuthorizationNecessary
            ));
            assert!(matches!(
                authorize(
                    #[cfg(feature = "authorization")]
                    &Method::Get,
                    "/oauth/callback",
                    Some("auth"),
                    &[Box::new(AlwaysRejectIdentityProvider)]
                ),
                AuthorizationResult::NoAuthorizationNecessary
            ));
        }
    }

    /// Verifies the simple case where `authorize` is called with a single identity provider that
    /// successfully gets the client's identity.
    #[test]
    fn authorize_successful_one_provider() {
        let expected_auth = "auth".parse().unwrap();
        let expected_identity = AlwaysAcceptIdentityProvider
            .get_identity(&expected_auth)
            .unwrap()
            .unwrap();

        assert!(matches!(
            authorize(
                #[cfg(feature = "authorization")]
                &Method::Get,
                "/test/endpoint",
                Some("auth"),
                &[Box::new(AlwaysAcceptIdentityProvider)]
            ),
            AuthorizationResult::Authorized(identity) if identity == expected_identity
        ));
    }

    /// Verifies the case where `authorize` is called with multile identity providers and only one
    /// successfully gets the client's identity.
    #[test]
    fn authorize_successful_multiple_providers() {
        let expected_auth = "auth".parse().unwrap();
        let expected_identity = AlwaysAcceptIdentityProvider
            .get_identity(&expected_auth)
            .unwrap()
            .unwrap();

        assert!(matches!(
            authorize(
                #[cfg(feature = "authorization")]
                &Method::Get,
                "/test/endpoint",
                Some("auth"),
                &[
                    Box::new(AlwaysRejectIdentityProvider),
                    Box::new(AlwaysAcceptIdentityProvider),
                    Box::new(AlwaysRejectIdentityProvider),
                ]
            ),
            AuthorizationResult::Authorized(identity) if identity == expected_identity
        ));
    }

    /// Verifies the case where `authorize` is called with multiple identity provider and one errors
    /// before another successfully gets the client's identity.
    #[test]
    fn authorize_successful_after_error() {
        let expected_auth = "auth".parse().unwrap();
        let expected_identity = AlwaysAcceptIdentityProvider
            .get_identity(&expected_auth)
            .unwrap()
            .unwrap();

        assert!(matches!(
            authorize(
                #[cfg(feature = "authorization")]
                &Method::Get,
                "/test/endpoint",
                Some("auth"),
                &[
                    Box::new(AlwaysErrIdentityProvider),
                    Box::new(AlwaysAcceptIdentityProvider),
                ]
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
}
