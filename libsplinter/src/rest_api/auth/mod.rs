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
pub mod actix;
pub mod identity;

use identity::{Authorization, IdentityProvider};

/// The possible outcomes of attempting to authorize a client
enum AuthorizationResult {
    /// The client was authorized to the given identity
    Authorized {
        identity: String,
        authorization: Authorization,
    },
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
/// * `endpoint` - The endpoint that is being requested. Example: "/endpoint/path"
/// * `auth_header` - The value of the Authorization HTTP header for the request
/// * `identity_providers` - The identity providers that will be used to check the client's identity
fn authorize(
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
            Ok(Some(identity)) => {
                return AuthorizationResult::Authorized {
                    identity,
                    authorization,
                }
            }
            Ok(None) => {}
            Err(err) => error!("{}", err),
        }
    }

    // No identity provider could resolve the authorization to an identity
    AuthorizationResult::Unauthorized
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::error::InternalError;

    /// Verifies that the `authorize` function returns `AuthorizationResult::Unauthorized` when no
    /// identity providers are specified. Without any identity providers, there's no way to properly
    /// authorize.
    #[test]
    fn authorize_no_identity_providers() {
        assert!(matches!(
            authorize("/test/endpoint", Some("auth"), &[]),
            AuthorizationResult::Unauthorized
        ));
    }

    /// Verifies that the `authorize` function returns `AuthorizationResult::Unauthorized` when no
    /// identity provider returns an identity for the auth header.
    #[test]
    fn authorize_no_matching_identity_provider() {
        assert!(matches!(
            authorize(
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
                "/test/endpoint",
                Some("auth"),
                &[Box::new(AlwaysAcceptIdentityProvider)]
            ),
            AuthorizationResult::Authorized {
                identity,
                authorization,
            } if identity == expected_identity && authorization == expected_auth
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
                "/test/endpoint",
                Some("auth"),
                &[
                    Box::new(AlwaysRejectIdentityProvider),
                    Box::new(AlwaysAcceptIdentityProvider),
                    Box::new(AlwaysRejectIdentityProvider),
                ]
            ),
            AuthorizationResult::Authorized {
                identity,
                authorization,
            } if identity == expected_identity && authorization == expected_auth
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
                "/test/endpoint",
                Some("auth"),
                &[
                    Box::new(AlwaysErrIdentityProvider),
                    Box::new(AlwaysAcceptIdentityProvider),
                ]
            ),
            AuthorizationResult::Authorized {
                identity,
                authorization,
            } if identity == expected_identity && authorization == expected_auth
        ));
    }

    /// An identity provider that always returns `Ok(Some("identity"))`
    #[derive(Clone)]
    struct AlwaysAcceptIdentityProvider;

    impl IdentityProvider for AlwaysAcceptIdentityProvider {
        fn get_identity(
            &self,
            _authorization: &Authorization,
        ) -> Result<Option<String>, InternalError> {
            Ok(Some("identity".into()))
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
            _authorization: &Authorization,
        ) -> Result<Option<String>, InternalError> {
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
            _authorization: &Authorization,
        ) -> Result<Option<String>, InternalError> {
            Err(InternalError::with_message("failed".into()))
        }

        fn clone_box(&self) -> Box<dyn IdentityProvider> {
            Box::new(self.clone())
        }
    }
}
