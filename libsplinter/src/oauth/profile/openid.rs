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

//! A profile provider that looks up OpenID profile information

use reqwest::{blocking::Client, StatusCode};
use serde::Deserialize;

use crate::error::InternalError;
use crate::oauth::Profile;

use super::ProfileProvider;

#[derive(Clone)]
pub struct OpenIdProfileProvider {
    userinfo_endpoint: String,
}

impl OpenIdProfileProvider {
    pub fn new(userinfo_endpoint: String) -> OpenIdProfileProvider {
        OpenIdProfileProvider { userinfo_endpoint }
    }
}

impl ProfileProvider for OpenIdProfileProvider {
    fn get_profile(&self, access_token: &str) -> Result<Option<Profile>, InternalError> {
        let response = Client::builder()
            .build()
            .map_err(|err| InternalError::from_source(err.into()))?
            .get(&self.userinfo_endpoint)
            .header("Authorization", format!("Bearer {}", access_token))
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

        let user_profile = response
            .json::<OpenIdProfileResponse>()
            .map_err(|_| InternalError::with_message("Received unexpected response body".into()))?;

        Ok(Some(Profile::from(user_profile)))
    }

    fn clone_box(&self) -> Box<dyn ProfileProvider> {
        Box::new(self.clone())
    }
}

#[derive(Debug, Deserialize)]
pub struct OpenIdProfileResponse {
    pub sub: String,
    pub name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub email: Option<String>,
    pub picture: Option<String>,
}

impl From<OpenIdProfileResponse> for Profile {
    fn from(openid_profile: OpenIdProfileResponse) -> Self {
        Profile {
            subject: openid_profile.sub,
            name: openid_profile.name,
            given_name: openid_profile.given_name,
            family_name: openid_profile.family_name,
            email: openid_profile.email,
            picture: openid_profile.picture,
        }
    }
}
