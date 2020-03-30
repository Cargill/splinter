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

use jsonwebtoken::{decode, Validation};

use crate::actix_web::HttpRequest;
use crate::biome::rest_api::resources::authorize::AuthorizationResult;
use crate::rest_api::get_authorization_token;
use crate::rest_api::secrets::SecretManager;
use crate::rest_api::sessions::Claims;

/// Verifies the user has the correct permissions
pub(crate) fn authorize_user<SM: SecretManager>(
    request: &HttpRequest,
    secret_manager: &SM,
    validation: &Validation,
) -> AuthorizationResult {
    let token = match get_authorization_token(&request) {
        Ok(token) => token,
        Err(err) => {
            debug!("Failed to get token: {}", err);
            return AuthorizationResult::Unauthorized("User is not authorized".to_string());
        }
    };

    validate_claims(&token, secret_manager, validation)
}

pub(crate) fn validate_claims<SM: SecretManager>(
    token: &str,
    secret_manager: &SM,
    validation: &Validation,
) -> AuthorizationResult {
    let secret = match secret_manager.secret() {
        Ok(secret) => secret,
        Err(err) => {
            debug!("Failed to fetch secret {}", err);
            return AuthorizationResult::Failed;
        }
    };

    match decode::<Claims>(&token, secret.as_ref(), validation) {
        Ok(claims) => AuthorizationResult::Authorized(claims.claims),
        Err(err) => {
            debug!("Invalid token: {}", err);
            AuthorizationResult::Unauthorized("User is not authorized".to_string())
        }
    }
}
