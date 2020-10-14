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

//! Support for OAuth2 authorization in Splinter

mod error;

use oauth2::basic::BasicClient;
use oauth2::{AuthUrl, ClientId, ClientSecret, TokenUrl};

pub use error::OAuthClientConfigurationError;

/// An OAuth2 client for Splinter
#[derive(Clone)]
pub struct OAuthClient {
    client: BasicClient,
    scopes: Vec<String>,
}

impl OAuthClient {
    pub fn new(
        client_id: String,
        client_secret: String,
        auth_url: String,
        token_url: String,
        scopes: Vec<String>,
    ) -> Result<Self, OAuthClientConfigurationError> {
        let client = BasicClient::new(
            ClientId::new(client_id),
            Some(ClientSecret::new(client_secret)),
            AuthUrl::new(auth_url)
                .map_err(|err| OAuthClientConfigurationError::InvalidAuthUrl(err.to_string()))?,
            Some(
                TokenUrl::new(token_url).map_err(|err| {
                    OAuthClientConfigurationError::InvalidTokenUrl(err.to_string())
                })?,
            ),
        );
        Ok(Self { client, scopes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that the `OAuthClient::new` is successful when valid URLs are provided but returns
    /// appropriate errors when invalid URLs are provided.
    #[test]
    fn client_construction() {
        OAuthClient::new(
            "client_id".into(),
            "client_secret".into(),
            "https://provider.com/auth".into(),
            "https://provider.com/token".into(),
            vec![],
        )
        .expect("Failed to create client from valid inputs");

        assert!(matches!(
            OAuthClient::new(
                "client_id".into(),
                "client_secret".into(),
                "invalid_auth_url".into(),
                "https://provider.com/token".into(),
                vec![],
            ),
            Err(OAuthClientConfigurationError::InvalidAuthUrl(_))
        ));

        assert!(matches!(
            OAuthClient::new(
                "client_id".into(),
                "client_secret".into(),
                "https://provider.com/auth".into(),
                "invalid_token_url".into(),
                vec![],
            ),
            Err(OAuthClientConfigurationError::InvalidTokenUrl(_))
        ));
    }
}
