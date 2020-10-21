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

//! Authentication and authorization tools for the Splinter REST API

#[cfg(feature = "rest-api-actix")]
pub mod actix;
pub mod identity;

use identity::{IdentityProvider, IdentityProviderError};

/// The possible outcomes of attempting to authorize a client
enum AuthorizationResult {
    /// The client was authorized to the given identity
    Authorized(String),
    /// The given authorization header isn't supported by any of the configured identity providers
    InvalidAuthorization(String),
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
    #[cfg(feature = "oauth")]
    if endpoint.starts_with("/oauth") {
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
        Err(_) => return AuthorizationResult::InvalidAuthorization(auth_str.into()),
    };

    // Attempt to get the client's identity
    let mut authorization_supported = false;
    for provider in identity_providers {
        match provider.get_identity(&authorization) {
            Ok(identity) => return AuthorizationResult::Authorized(identity),
            Err(IdentityProviderError::Unauthorized) => authorization_supported = true,
            Err(IdentityProviderError::UnsupportedAuth) => {}
            Err(err) => error!("{}", err),
        }
    }

    // If no auth was successful, determine if it was an unsupported type or just not
    // valid for any of the providers
    if authorization_supported {
        AuthorizationResult::Unauthorized
    } else {
        AuthorizationResult::InvalidAuthorization(auth_str.into())
    }
}
