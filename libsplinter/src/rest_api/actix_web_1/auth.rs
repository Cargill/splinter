// Copyright 2018-2021 Cargill Incorporated
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

use actix_web::HttpRequest;
#[cfg(feature = "cylinder-jwt")]
use cylinder::Verifier;

#[cfg(feature = "biome-credentials")]
use crate::biome::credentials::rest_api::BiomeCredentialsRestResourceProvider;
#[cfg(feature = "oauth")]
use crate::biome::OAuthUserSessionStore;
#[cfg(all(feature = "oauth", feature = "biome-profile"))]
use crate::biome::UserProfileStore;
use crate::rest_api::auth::identity::IdentityProvider;
#[cfg(feature = "oauth")]
use crate::rest_api::OAuthConfig;

use super::{RequestError, Resource};

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
        resources: Vec<Resource>,
        /// The identity provider that correlates the contents of the `Authorization` header with
        /// an identity for the client
        identity_provider: Box<dyn IdentityProvider>,
    },
}

pub fn require_header(header_key: &str, request: &HttpRequest) -> Result<String, RequestError> {
    let header = request.headers().get(header_key).ok_or_else(|| {
        RequestError::MissingHeader(format!("Header {} not included in Request", header_key))
    })?;
    Ok(header
        .to_str()
        .map_err(|err| RequestError::InvalidHeaderValue(format!("Invalid header value: {}", err)))?
        .to_string())
}

pub fn get_authorization_token(request: &HttpRequest) -> Result<String, RequestError> {
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
