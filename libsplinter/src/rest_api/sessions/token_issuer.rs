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

//! Provides an implementation of a TokenIssuer

use std::sync::Arc;

use jsonwebtoken::{encode, Header};

use super::{Claims, TokenIssuer, TokenIssuerError};
use crate::rest_api::secrets::SecretManager;

/// Issues JWT access tokens
pub struct AccessTokenIssuer {
    secret_manager: Arc<dyn SecretManager>,
    #[cfg(feature = "biome-credentials")]
    refresh_secret_manager: Arc<dyn SecretManager>,
}

impl AccessTokenIssuer {
    /// Creates a new AccessTokenIssuer that will use the given secret manager for issuing tokens
    pub fn new(
        secret_manager: Arc<dyn SecretManager>,
        #[cfg(feature = "biome-credentials")] refresh_secret_manager: Arc<dyn SecretManager>,
    ) -> AccessTokenIssuer {
        AccessTokenIssuer {
            secret_manager,
            #[cfg(feature = "biome-credentials")]
            refresh_secret_manager,
        }
    }
}

impl TokenIssuer<Claims> for AccessTokenIssuer {
    fn issue_token_with_claims(&self, claims: Claims) -> Result<String, TokenIssuerError> {
        let token = encode(
            &Header::default(),
            &claims,
            self.secret_manager.secret()?.as_ref(),
        )?;
        Ok(token)
    }

    #[cfg(feature = "biome-credentials")]
    fn issue_refresh_token_with_claims(&self, claims: Claims) -> Result<String, TokenIssuerError> {
        let token = encode(
            &Header::default(),
            &claims,
            self.refresh_secret_manager.secret()?.as_ref(),
        )?;
        Ok(token)
    }
}
