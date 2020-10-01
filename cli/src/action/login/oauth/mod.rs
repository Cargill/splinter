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

mod callback;
mod error;
mod provider;

use std::path::PathBuf;

use crate::error::CliError;

/// Contains the user information returned by an OAuth2 Provider.
pub struct UserTokens {
    pub provider_type: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
}

impl std::fmt::Debug for UserTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("UserTokens")
            .field("provider_type", &self.provider_type)
            .field("access_token", &"<Redacted>".to_string())
            .field(
                "refresh_token",
                &self.refresh_token.as_deref().map(|_| "<Redacted>"),
            )
            .finish()
    }
}

/// Handles an OAuth2 login command.
pub fn handle_oauth2_login(base_url: &str, user_splinter_dir: PathBuf) -> Result<(), CliError> {
    let provider_details = get_provider_details(base_url)?;

    let provider = provider::Provider::new(
        provider_details.provider_id,
        provider_details.client_id,
        provider_details.client_secret,
        provider_details.auth_url,
        provider_details.token_url,
        provider_details.scopes,
    );

    let tokens = provider
        .get_tokens()
        .map_err(|err| CliError::ActionError(err.to_string()))?;


    Ok(())
}

fn get_provider_details(_base_url: &str) -> Result<ProviderDetails, CliError> {
    // This configuration based on environment variables is a temporary placeholder configuration,
    // where the final configuration will be provided by the Splinter REST API.
    let client_id = option_env!("OAUTH2_CLIENT_ID")
        .map(|s| s.to_string())
        .ok_or_else(|| {
            CliError::ActionError(
                "This binary was not properly configured at build-time for OAuth2 login; \
                missing client id"
                    .into(),
            )
        })?;
    let client_secret = option_env!("OAUTH2_CLIENT_SECRET")
        .map(|s| s.to_string())
        .ok_or_else(|| {
            CliError::ActionError(
                "This binary was not properly configured at build-time for OAuth2 login; \
                missing client secret"
                    .into(),
            )
        })?;

    let provider_id = option_env!("OAUTH2_PROVIDER_ID")
        .map(|s| s.to_string())
        .ok_or_else(|| {
            CliError::ActionError(
                "This binary was not properly configured at build-time for OAuth2 login; \
                missing provider id"
                    .into(),
            )
        })?;

    let auth_url = option_env!("OAUTH2_AUTH_URL")
        .map(|s| s.to_string())
        .ok_or_else(|| {
            CliError::ActionError(
                "This binary was not properly configured at build-time for OAuth2 login; \
                missing auth URL"
                    .into(),
            )
        })?;
    let token_url = option_env!("OAUTH2_TOKEN_URL")
        .map(|s| s.to_string())
        .ok_or_else(|| {
            CliError::ActionError(
                "This binary was not properly configured at build-time for OAuth2 login; \
                missing token URL"
                    .into(),
            )
        })?;

    let scopes = option_env!("OAUTH2_SCOPES")
        .map(|s| s.split(',').map(ToString::to_string).collect::<Vec<_>>())
        .ok_or_else(|| {
            CliError::ActionError(
                "This binary was not properly configured at build-time for OAuth2 login; \
                missing scopes"
                    .into(),
            )
        })?;

    Ok(ProviderDetails {
        provider_id,
        client_id,
        client_secret,
        auth_url,
        token_url,
        scopes,
    })
}

#[derive(serde::Deserialize)]
struct ProviderDetails {
    provider_id: String,
    client_id: String,
    client_secret: String,
    auth_url: String,
    token_url: String,
    scopes: Vec<String>,
}
