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

use crate::auth::{
    oauth::{rest_api::OAuthUserInfoStore, UserInfo},
    rest_api::identity::{Authorization, AuthorizationMapping, BearerToken},
};
use crate::biome::oauth::store::{AccessToken, OAuthProvider, OAuthUserBuilder, OAuthUserStore};
use crate::biome::user::store::{User, UserStore};
use crate::error::InternalError;

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

/// A wrapper struct for an `OAuthUser`'s identity.
pub struct OAuthUserIdentityRef(pub String);

/// An `AuthorizationMapping` implementation that returns  an `OAuthUser`'s identity.
pub struct GetUserIdentityByOAuthAuthorization {
    oauth_user_store: Box<dyn OAuthUserStore>,
}

impl GetUserIdentityByOAuthAuthorization {
    /// Construct a new `GetUserIdentityByOAuthAuthorization` over an `OAuthUserStore` implementation.
    pub fn new(oauth_user_store: Box<dyn OAuthUserStore>) -> Self {
        Self { oauth_user_store }
    }
}

impl AuthorizationMapping<OAuthUserIdentityRef> for GetUserIdentityByOAuthAuthorization {
    fn get(
        &self,
        authorization: &Authorization,
    ) -> Result<Option<OAuthUserIdentityRef>, InternalError> {
        match authorization {
            Authorization::Bearer(BearerToken::OAuth2(access_token)) => self
                .oauth_user_store
                .get_by_access_token(&access_token)
                .map(|opt_oauth_user| {
                    opt_oauth_user.map(|oauth_user| {
                        OAuthUserIdentityRef(oauth_user.provider_user_ref().to_string())
                    })
                })
                .map_err(|e| {
                    InternalError::from_source_with_message(
                        Box::new(e),
                        "Unable to load oauth user".into(),
                    )
                }),
            _ => Ok(None),
        }
    }
}

/// Biome-backed implementation of the `OAuthUserInfoStore` trait.
///
/// This implementation uses the `OAuthUserStore` provided by Biome.
#[derive(Clone)]
pub struct BiomeOAuthUserInfoStore {
    provider: OAuthProvider,
    user_store: Box<dyn UserStore>,
    oauth_user_store: Box<dyn OAuthUserStore>,
}

impl BiomeOAuthUserInfoStore {
    /// Construct a new `BiomeOAuthUserInfoStore`.
    pub fn new(
        provider: OAuthProvider,
        user_store: Box<dyn UserStore>,
        oauth_user_store: Box<dyn OAuthUserStore>,
    ) -> Self {
        Self {
            provider,
            user_store,
            oauth_user_store,
        }
    }
}

impl OAuthUserInfoStore for BiomeOAuthUserInfoStore {
    fn save_user_info(&self, user_info: &UserInfo) -> Result<(), InternalError> {
        let provider_identity = user_info.identity().to_string();

        let existing_oauth_user = self
            .oauth_user_store
            .get_by_provider_user_ref(&provider_identity)
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        if let Some(oauth_user) = existing_oauth_user {
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
        } else {
            let user_id = Uuid::new_v4().to_string();
            let user = User::new(&user_id);

            self.user_store
                .add_user(user)
                .map_err(|e| InternalError::from_source(Box::new(e)))?;

            let oauth_user = OAuthUserBuilder::new()
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
                .map_err(|e| InternalError::from_source(Box::new(e)))?;
        }

        Ok(())
    }

    fn remove_user_tokens(&self, identity: &str) -> Result<(), InternalError> {
        // Check if there is an existing `OAuthUser` with the corresponding `identity`
        if let Some(oauth_user) = self
            .oauth_user_store
            .get_by_provider_user_ref(&identity)
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
