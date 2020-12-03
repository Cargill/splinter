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

//! An identity provider that extracts the user ID from a Biome JWT

use std::sync::Arc;

use jsonwebtoken::{decode, Validation};

use crate::error::InternalError;
use crate::rest_api::{secrets::SecretManager, sessions::Claims};

use super::{Authorization, BearerToken, IdentityProvider};

/// Extracts the user ID from a Biome JWT
///
/// This provider only accepts `Authorization::Bearer(BearerToken::Biome(token))` authorizations,
/// and the inner token must be a valid Biome JWT.
#[derive(Clone)]
pub struct BiomeUserIdentityProvider {
    token_secret_manager: Arc<dyn SecretManager>,
    validation: Validation,
}

impl BiomeUserIdentityProvider {
    /// Creates a new Biome user identity provider
    pub fn new(token_secret_manager: Arc<dyn SecretManager>, validation: Validation) -> Self {
        Self {
            token_secret_manager,
            validation,
        }
    }
}

impl IdentityProvider for BiomeUserIdentityProvider {
    fn get_identity(&self, authorization: &Authorization) -> Result<Option<String>, InternalError> {
        let token = match authorization {
            Authorization::Bearer(BearerToken::Biome(token)) => token,
            _ => return Ok(None),
        };

        let secret = self
            .token_secret_manager
            .secret()
            .map_err(|err| InternalError::from_source(err.into()))?;

        Ok(decode::<Claims>(&token, secret.as_ref(), &self.validation)
            .map(|token_data| token_data.claims.user_id())
            .ok())
    }

    fn clone_box(&self) -> Box<dyn IdentityProvider> {
        Box::new(self.clone())
    }
}
