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

//! A subject provider that looks up GitHub usernames

use reqwest::{blocking::Client, StatusCode};
use serde::Deserialize;

use crate::error::InternalError;

use super::SubjectProvider;

/// Retrieves a GitHub username from the GitHub servers
#[derive(Clone)]
pub struct GithubSubjectProvider;

impl SubjectProvider for GithubSubjectProvider {
    fn get_subject(&self, access_token: &str) -> Result<Option<String>, InternalError> {
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

        let username = response
            .json::<UserResponse>()
            .map_err(|_| InternalError::with_message("Received unexpected response body".into()))?
            .login;

        Ok(Some(username))
    }

    fn clone_box(&self) -> Box<dyn SubjectProvider> {
        Box::new(self.clone())
    }
}

/// Deserializes the GitHub response
#[derive(Debug, Deserialize)]
struct UserResponse {
    login: String,
}
