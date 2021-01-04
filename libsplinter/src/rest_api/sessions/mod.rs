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

//! Provides an API for managing user sessions, including issuing and validating JWT tokens

mod claims;
mod error;
mod token_issuer;

#[cfg(any(feature = "biome-key-management", feature = "biome-credentials",))]
use jsonwebtoken::Validation;
use serde::Serialize;

pub use claims::{Claims, ClaimsBuilder};
pub use error::{ClaimsBuildError, TokenIssuerError, TokenValidationError};
pub use token_issuer::AccessTokenIssuer;

#[cfg(any(feature = "biome-key-management", feature = "biome-credentials",))]
const DEFAULT_LEEWAY: i64 = 10; // default leeway in seconds.

/// Implementers can issue JWT tokens
pub trait TokenIssuer<T: Serialize> {
    /// Issues a JWT token with the given claims
    fn issue_token_with_claims(&self, claims: T) -> Result<String, TokenIssuerError>;

    #[cfg(feature = "biome-credentials")]
    fn issue_refresh_token_with_claims(&self, claims: T) -> Result<String, TokenIssuerError>;
}

#[cfg(any(feature = "biome-key-management", feature = "biome-credentials",))]
pub(crate) fn default_validation(issuer: &str) -> Validation {
    Validation {
        leeway: DEFAULT_LEEWAY,
        iss: Some(issuer.to_string()),
        ..Default::default()
    }
}

/// Validates authorization token but ignores the expiration date
#[cfg(feature = "biome-credentials")]
pub(crate) fn ignore_exp_validation(issuer: &str) -> Validation {
    Validation {
        leeway: DEFAULT_LEEWAY,
        iss: Some(issuer.to_string()),
        validate_exp: false,
        ..Default::default()
    }
}
