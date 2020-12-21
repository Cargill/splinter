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

use reqwest::{blocking::Client, StatusCode};

use crate::error::InternalError;

use super::{AuthorizationHeader, BearerToken, IdentityProvider};

#[derive(Clone)]
pub struct OpenIdUserIdentityProvider {
    userinfo_endpoint: String,
}

impl OpenIdUserIdentityProvider {
    pub fn new(userinfo_endpoint: String) -> OpenIdUserIdentityProvider {
        OpenIdUserIdentityProvider { userinfo_endpoint }
    }
}

impl IdentityProvider for OpenIdUserIdentityProvider {
    fn get_identity(
        &self,
        authorization: &AuthorizationHeader,
    ) -> Result<Option<String>, InternalError> {
        let token = match authorization {
            AuthorizationHeader::Bearer(BearerToken::OAuth2(token)) => token,
            _ => return Ok(None),
        };

        let response = Client::builder()
            .build()
            .map_err(|err| InternalError::from_source(err.into()))?
            .get(&self.userinfo_endpoint)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .map_err(|err| InternalError::from_source(err.into()))?;

        if !response.status().is_success() {
            match response.status() {
                StatusCode::UNAUTHORIZED => return Ok(None),
                status_code => {
                    return Err(InternalError::with_message(format!(
                        "Received unexpected response code: {}",
                        status_code
                    )))
                }
            }
        }

        let user_identity = response
            .json::<UserResponse>()
            .map_err(|_| InternalError::with_message("Received unexpected response body".into()))?
            .sub;

        Ok(Some(user_identity))
    }

    fn clone_box(&self) -> Box<dyn IdentityProvider> {
        Box::new(self.clone())
    }
}

/// Deserializes response
#[derive(Debug, Deserialize)]
struct UserResponse {
    sub: String,
}
