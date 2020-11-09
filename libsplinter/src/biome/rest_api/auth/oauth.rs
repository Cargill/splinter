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

//! SaveTokenOperation implementation, backed by Biome's OAuthUserStore.

use uuid::Uuid;

use crate::auth::{
    oauth::{rest_api::SaveTokensOperation, UserTokens},
    rest_api::identity::{Authorization, BearerToken, IdentityProvider},
};
use crate::biome::oauth::store::{OAuthProvider, OAuthUserBuilder, OAuthUserStore};
use crate::biome::user::store::{User, UserStore};
use crate::error::InternalError;

/// Biome-backed implementation of the SaveTokensOperation trait.
///
/// This implementation stores the UserToken values using the OAuthUserStore provided by Biome.
#[derive(Clone)]
pub struct OAuthUserStoreSaveTokensOperation {
    provider: OAuthProvider,
    identity_provider: Box<dyn IdentityProvider>,
    user_store: Box<dyn UserStore>,
    oauth_user_store: Box<dyn OAuthUserStore>,
}

impl OAuthUserStoreSaveTokensOperation {
    /// Construct a new OAuthUserStoreSaveTokensOperation.
    pub fn new(
        provider: OAuthProvider,
        identity_provider: Box<dyn IdentityProvider>,
        user_store: Box<dyn UserStore>,
        oauth_user_store: Box<dyn OAuthUserStore>,
    ) -> Self {
        Self {
            provider,
            identity_provider,
            user_store,
            oauth_user_store,
        }
    }
}

impl SaveTokensOperation for OAuthUserStoreSaveTokensOperation {
    fn save_tokens(&self, user_tokens: &UserTokens) -> Result<(), InternalError> {
        let authorization =
            Authorization::Bearer(BearerToken::OAuth2(user_tokens.access_token().to_string()));

        let provider_identity = self
            .identity_provider
            .get_identity(&authorization)
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        let existing_oauth_user = self
            .oauth_user_store
            .get_by_provider_user_ref(&provider_identity)
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        if let Some(oauth_user) = existing_oauth_user {
            let updated_user = oauth_user
                .into_update_builder()
                .with_access_token(user_tokens.access_token().into())
                .with_refresh_token(user_tokens.refresh_token().map(String::from))
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
        } else {
            let user_id = Uuid::new_v4().to_string();
            let user = User::new(&user_id);

            self.user_store
                .add_user(user)
                .map_err(|e| InternalError::from_source(Box::new(e)))?;

            let oauth_user = OAuthUserBuilder::new()
                .with_user_id(user_id)
                .with_provider_user_ref(provider_identity)
                .with_access_token(user_tokens.access_token().into())
                .with_refresh_token(user_tokens.refresh_token().map(String::from))
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
                .map_err(|e| InternalError::from_source(Box::new(e)))?;
        }

        Ok(())
    }

    fn clone_box(&self) -> Box<dyn SaveTokensOperation> {
        Box::new(self.clone())
    }
}
