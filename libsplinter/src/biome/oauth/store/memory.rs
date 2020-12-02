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

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::error::InternalError;

use super::{
    AccessToken, NewOAuthUserAccess, OAuthUserAccess, OAuthUserStore, OAuthUserStoreError,
};

#[derive(Default, Clone)]
pub struct MemoryOAuthUserStore {
    inner: Arc<Mutex<HashMap<String, OAuthUserAccess>>>,
}

impl MemoryOAuthUserStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl OAuthUserStore for MemoryOAuthUserStore {
    fn add_oauth_user(&self, oauth_user: NewOAuthUserAccess) -> Result<(), OAuthUserStoreError> {
        let mut inner = self.inner.lock().map_err(|_| {
            OAuthUserStoreError::InternalError(InternalError::with_message(
                "Cannot access OAuth user store: mutex lock poisoned".to_string(),
            ))
        })?;

        let NewOAuthUserAccess {
            user_id,
            provider_user_ref,
            access_token,
            refresh_token,
            provider,
        } = oauth_user;

        let oauth_user_access = OAuthUserAccess {
            user_id,
            provider_user_ref,
            access_token,
            refresh_token,
            provider,
        };
        inner.insert(oauth_user_access.user_id().to_string(), oauth_user_access);
        Ok(())
    }

    fn update_oauth_user(&self, oauth_user: OAuthUserAccess) -> Result<(), OAuthUserStoreError> {
        let mut inner = self.inner.lock().map_err(|_| {
            OAuthUserStoreError::InternalError(InternalError::with_message(
                "Cannot access OAuth user store: mutex lock poisoned".to_string(),
            ))
        })?;
        inner.insert(oauth_user.user_id().to_string(), oauth_user);
        Ok(())
    }

    fn get_by_provider_user_ref(
        &self,
        provider_user_ref: &str,
    ) -> Result<Option<OAuthUserAccess>, OAuthUserStoreError> {
        let inner = self.inner.lock().map_err(|_| {
            OAuthUserStoreError::InternalError(InternalError::with_message(
                "Cannot access OAuth user store: mutex lock poisoned".to_string(),
            ))
        })?;

        Ok(inner
            .values()
            .find(|oauth_user| oauth_user.provider_user_ref() == provider_user_ref)
            .cloned())
    }

    fn get_by_access_token(
        &self,
        access_token: &str,
    ) -> Result<Option<OAuthUserAccess>, OAuthUserStoreError> {
        let inner = self.inner.lock().map_err(|_| {
            OAuthUserStoreError::InternalError(InternalError::with_message(
                "Cannot access OAuth user store: mutex lock poisoned".to_string(),
            ))
        })?;

        Ok(inner
            .values()
            .find(|oauth_user| {
                oauth_user.access_token() == &AccessToken::Authorized(access_token.to_string())
            })
            .cloned())
    }

    fn get_by_user_id(
        &self,
        user_id: &str,
    ) -> Result<Option<OAuthUserAccess>, OAuthUserStoreError> {
        let inner = self.inner.lock().map_err(|_| {
            OAuthUserStoreError::InternalError(InternalError::with_message(
                "Cannot access OAuth user store: mutex lock poisoned".to_string(),
            ))
        })?;

        Ok(inner.get(user_id).cloned())
    }

    fn clone_box(&self) -> Box<dyn OAuthUserStore> {
        Box::new(self.clone())
    }
}
