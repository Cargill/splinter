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

//! SaveTokenOperation implementation, backed by Biome's OAuthUserSessionStore. It also includes
//! an AuthorizationMapping implementation for use with OAuth2 bearer tokens.

use crate::biome::{
    oauth::store::{InsertableOAuthUserSessionBuilder, OAuthUserSessionStore},
    rest_api::resources::User,
};
use crate::error::InternalError;
use crate::oauth::{rest_api::OAuthUserInfoStore, UserInfo};
use crate::rest_api::auth::{AuthorizationHeader, AuthorizationMapping, BearerToken};

/// An `AuthorizationMapping` implementation that returns an `User`.
pub struct GetUserByOAuthAuthorization {
    oauth_user_session_store: Box<dyn OAuthUserSessionStore>,
}

impl GetUserByOAuthAuthorization {
    /// Construct a new `GetUserByOAuthAuthorization` over an `OAuthUserSessionStore` implementation.
    pub fn new(oauth_user_session_store: Box<dyn OAuthUserSessionStore>) -> Self {
        Self {
            oauth_user_session_store,
        }
    }
}

impl AuthorizationMapping<User> for GetUserByOAuthAuthorization {
    fn get(&self, authorization: &AuthorizationHeader) -> Result<Option<User>, InternalError> {
        match authorization {
            AuthorizationHeader::Bearer(BearerToken::OAuth2(access_token)) => {
                debug!("Getting user for access token {}", access_token);
                self.oauth_user_session_store
                    .get_session(&access_token)
                    .map(|opt_session| {
                        opt_session.map(|session| User::new(session.user().user_id()))
                    })
                    .map_err(|e| {
                        InternalError::from_source_with_message(
                            Box::new(e),
                            "Unable to load oauth session".into(),
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
    oauth_user_session_store: Box<dyn OAuthUserSessionStore>,
}

impl BiomeOAuthUserInfoStore {
    /// Construct a new `BiomeOAuthUserInfoStore`.
    pub fn new(oauth_user_session_store: Box<dyn OAuthUserSessionStore>) -> Self {
        Self {
            oauth_user_session_store,
        }
    }
}

impl OAuthUserInfoStore for BiomeOAuthUserInfoStore {
    fn save_user_info(
        &self,
        splinter_access_token: String,
        user_info: &UserInfo,
    ) -> Result<(), InternalError> {
        InsertableOAuthUserSessionBuilder::new()
            .with_splinter_access_token(splinter_access_token)
            .with_subject(user_info.identity().to_string())
            .with_oauth_access_token(user_info.access_token().to_string())
            .with_oauth_refresh_token(user_info.refresh_token().map(ToOwned::to_owned))
            .build()
            .map_err(|e| InternalError::from_source(Box::new(e)))
            .and_then(|session| {
                self.oauth_user_session_store
                    .add_session(session)
                    .map_err(|e| InternalError::from_source(Box::new(e)))
            })
    }

    fn remove_user_tokens(&self, access_token: &str) -> Result<(), InternalError> {
        self.oauth_user_session_store
            .remove_session(access_token)
            .map_err(|e| InternalError::from_source(Box::new(e)))
    }

    fn clone_box(&self) -> Box<dyn OAuthUserInfoStore> {
        Box::new(self.clone())
    }
}
