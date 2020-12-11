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

//! SaveTokenOperation implementation, backed by Biome's OAuthUserStore. It also includes
//! an AuthorizationMapping implementation for use with OAuth2 bearer tokens.

use uuid::Uuid;

use crate::biome::{
    oauth::store::{AccessToken, NewOAuthUserAccessBuilder, OAuthProvider, OAuthUserStore},
    rest_api::resources::User,
};
use crate::error::InternalError;
use crate::oauth::{rest_api::OAuthUserInfoStore, UserInfo};
use crate::rest_api::auth::identity::{Authorization, AuthorizationMapping, BearerToken};

/// An `AuthorizationMapping` implementation that returns an `User`.
pub struct GetUserByOAuthAuthorization {
    oauth_user_store: Box<dyn OAuthUserStore>,
}

impl GetUserByOAuthAuthorization {
    /// Construct a new `GetUserByOAuthAuthorization` over an `OAuthUserStore` implementation.
    pub fn new(oauth_user_store: Box<dyn OAuthUserStore>) -> Self {
        Self { oauth_user_store }
    }
}

impl AuthorizationMapping<User> for GetUserByOAuthAuthorization {
    fn get(&self, authorization: &Authorization) -> Result<Option<User>, InternalError> {
        match authorization {
            Authorization::Bearer(BearerToken::OAuth2(access_token)) => {
                debug!("Getting user for access token {}", access_token);
                self.oauth_user_store
                    .get_by_access_token(&access_token)
                    .map(|opt_oauth_user| {
                        opt_oauth_user.map(|oauth_user| User::new(oauth_user.user_id()))
                    })
                    .map_err(|e| {
                        InternalError::from_source_with_message(
                            Box::new(e),
                            "Unable to load oauth user".into(),
                        )
                    })
            }
            _ => Ok(None),
        }
    }
}

/// Biome-backed implementation of the `OAuthUserInfoStore` trait.
#[derive(Clone)]
pub struct BiomeOAuthUserInfoStore {
    provider: OAuthProvider,
    oauth_user_store: Box<dyn OAuthUserStore>,
}

impl BiomeOAuthUserInfoStore {
    /// Construct a new `BiomeOAuthUserInfoStore`.
    pub fn new(provider: OAuthProvider, oauth_user_store: Box<dyn OAuthUserStore>) -> Self {
        Self {
            provider,
            oauth_user_store,
        }
    }
}

impl OAuthUserInfoStore for BiomeOAuthUserInfoStore {
    fn save_user_info(&self, user_info: &UserInfo) -> Result<(), InternalError> {
        let provider_identity = user_info.identity().to_string();

        let (previously_unauthed, other_accesses): (Vec<_>, Vec<_>) = self
            .oauth_user_store
            .list_by_provider_user_ref(&provider_identity)
            .map_err(|e| InternalError::from_source(Box::new(e)))?
            .partition(|oauth_user| oauth_user.access_token().is_unauthorized());

        // Convert the first found entry with no access token to use this access token
        if let Some(oauth_user) = previously_unauthed.into_iter().next() {
            let updated_user = oauth_user
                .into_update_builder()
                .with_access_token(AccessToken::Authorized(
                    user_info.access_token().to_string(),
                ))
                .with_refresh_token(user_info.refresh_token().map(String::from))
                .build()
                .map_err(|e| {
                    InternalError::from_source_with_message(
                        Box::new(e),
                        "Failed to properly construct an updated OAuth user".into(),
                    )
                })?;

            self.oauth_user_store
                .update_oauth_user(updated_user)
                .map_err(|e| InternalError::from_source(Box::new(e)))?;

            return Ok(());
        }

        // If there is an existing connection, maintain the existing linkage
        let user_id = if let Some(oauth_user) = other_accesses.into_iter().next() {
            oauth_user.user_id().to_string()
        } else {
            // otherwise, create a new user
            Uuid::new_v4().to_string()
        };

        let oauth_user = NewOAuthUserAccessBuilder::new()
            .with_user_id(user_id)
            .with_provider_user_ref(provider_identity)
            .with_access_token(AccessToken::Authorized(
                user_info.access_token().to_string(),
            ))
            .with_refresh_token(user_info.refresh_token().map(String::from))
            .with_provider(self.provider.clone())
            .build()
            .map_err(|e| {
                InternalError::from_source_with_message(
                    Box::new(e),
                    "Failed to properly construct a new OAuth user".into(),
                )
            })?;

        self.oauth_user_store
            .add_oauth_user(oauth_user)
            .map_err(|e| InternalError::from_source(Box::new(e)))
    }

    fn remove_user_tokens(&self, access_token: &str) -> Result<(), InternalError> {
        // Check if there is an existing `OAuthUserAccess` with the corresponding `identity`
        if let Some(oauth_user) = self
            .oauth_user_store
            .get_by_access_token(access_token)
            .map_err(|e| InternalError::from_source(Box::new(e)))?
        {
            // If the user does exist, remove any tokens associated with the user
            let updated_user = oauth_user
                .into_update_builder()
                .with_access_token(AccessToken::Unauthorized)
                .with_refresh_token(None)
                .build()
                .map_err(|e| {
                    InternalError::from_source_with_message(
                        Box::new(e),
                        "Failed to properly construct an updated OAuth user".into(),
                    )
                })?;
            self.oauth_user_store
                .update_oauth_user(updated_user)
                .map_err(|e| InternalError::from_source(Box::new(e)))?;
        }
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn OAuthUserInfoStore> {
        Box::new(self.clone())
    }
}
