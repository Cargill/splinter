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

//! An AuthorizationMapping implementation for use with Biome bearer tokens.

use std::sync::Arc;

use jsonwebtoken::decode;

use crate::biome::rest_api::BiomeRestConfig;
use crate::biome::user::store::User;
use crate::error::InternalError;
use crate::rest_api::{
    auth::identity::{Authorization, AuthorizationMapping, BearerToken},
    secrets::SecretManager,
    sessions::{default_validation, Claims},
};

/// An `AuthorizationMapping` implementation that returns an `User`.
///
/// This mapping gets a User based on a Biome authorization token.
pub struct GetUserByBiomeAuthorization {
    rest_config: Arc<BiomeRestConfig>,
    secret_manager: Arc<dyn SecretManager>,
}

impl GetUserByBiomeAuthorization {
    /// Constructs a new `GetUserByBiomeAuthorization` with the REST configuation and a secret
    /// manager.
    pub fn new(rest_config: Arc<BiomeRestConfig>, secret_manager: Arc<dyn SecretManager>) -> Self {
        Self {
            rest_config,
            secret_manager,
        }
    }
}

impl AuthorizationMapping<User> for GetUserByBiomeAuthorization {
    fn get(&self, authorization: &Authorization) -> Result<Option<User>, InternalError> {
        match authorization {
            Authorization::Bearer(BearerToken::Biome(token)) => {
                let validation = default_validation(&self.rest_config.issuer());
                let secret = match self.secret_manager.secret() {
                    Ok(secret) => secret,
                    Err(err) => {
                        return Err(InternalError::from_source(Box::new(err)));
                    }
                };

                match decode::<Claims>(&token, secret.as_ref(), &validation) {
                    Ok(claims) => Ok(Some(User::new(&claims.claims.user_id()))),
                    Err(err) => Err(InternalError::from_source(Box::new(err))),
                }
            }
            _ => Ok(None),
        }
    }
}
