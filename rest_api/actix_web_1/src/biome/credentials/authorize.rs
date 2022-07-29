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

use std::sync::Arc;

use actix_web::HttpRequest;
#[cfg(feature = "biome-credentials")]
use jsonwebtoken::{decode, DecodingKey, Validation};

#[cfg(feature = "biome-credentials")]
use crate::biome::credentials::resources::authorize::AuthorizationResult;
use splinter_rest_api_common::secrets::SecretManager;
#[cfg(feature = "biome-credentials")]
use splinter_rest_api_common::sessions::Claims;

use crate::framework::get_authorization_token;

/// Verifies the user has the correct permissions
#[cfg(feature = "biome-credentials")]
pub(crate) fn authorize_user(
    request: &HttpRequest,
    secret_manager: &Arc<dyn SecretManager>,
    validation: &Validation,
) -> AuthorizationResult {
    let token = match get_authorization_token(request) {
        Ok(token) => match token.split_once(':').map(|x| x.1) {
            Some(token) => token.to_string(),
            None => {
                debug!("Invalid token; should be in the format 'Biome:<JWT>'");
                return AuthorizationResult::Unauthorized;
            }
        },
        Err(err) => {
            debug!("Failed to get token: {}", err);
            return AuthorizationResult::Unauthorized;
        }
    };

    validate_claims(&token, secret_manager, validation)
}

#[cfg(feature = "biome-credentials")]
pub(crate) fn validate_claims(
    token: &str,
    secret_manager: &Arc<dyn SecretManager>,
    validation: &Validation,
) -> AuthorizationResult {
    let secret = match secret_manager.secret() {
        Ok(secret) => secret,
        Err(err) => {
            debug!("Failed to fetch secret {}", err);
            return AuthorizationResult::Failed;
        }
    };

    match decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        validation,
    ) {
        Ok(claims) => AuthorizationResult::Authorized(claims.claims),
        Err(err) => {
            debug!("Invalid token: {}", err);
            AuthorizationResult::Unauthorized
        }
    }
}
