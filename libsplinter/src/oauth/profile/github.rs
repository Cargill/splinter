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

//! A profile provider that looks up GitHub profile information

use reqwest::{blocking::Client, StatusCode};
use serde::Deserialize;

use crate::error::InternalError;
use crate::oauth::Profile;

use super::ProfileProvider;

/// Retrieves a GitHub profile information from the GitHub servers
#[derive(Clone)]
pub struct GithubProfileProvider;

impl ProfileProvider for GithubProfileProvider {
    fn get_profile(&self, access_token: &str) -> Result<Option<Profile>, InternalError> {
        let response = Client::builder()
            .build()
            .map_err(|err| InternalError::from_source(err.into()))?
            .get("https://api.github.com/user")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "splinter")
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
            .json::<GithubProfileResponse>()
            .map_err(|_| InternalError::with_message("Received unexpected response body".into()))?;

        Ok(Some(Profile::from(user_profile)))
    }

    fn clone_box(&self) -> Box<dyn ProfileProvider> {
        Box::new(self.clone())
    }
}

#[derive(Debug, Deserialize)]
pub struct GithubProfileResponse {
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}

impl From<GithubProfileResponse> for Profile {
    fn from(github_profile: GithubProfileResponse) -> Self {
        Profile {
            subject: github_profile.login,
            name: github_profile.name,
            given_name: None,
            family_name: None,
            email: github_profile.email,
            picture: github_profile.avatar_url,
        }
    }
}
