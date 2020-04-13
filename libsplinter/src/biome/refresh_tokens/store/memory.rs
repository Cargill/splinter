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

use crate::biome::refresh_tokens::store::{error::RefreshTokenError, RefreshTokenStore};

#[derive(Default, Clone)]
pub struct MemoryRefreshTokenStore {
    inner: Arc<Mutex<HashMap<String, String>>>,
}

impl MemoryRefreshTokenStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl RefreshTokenStore for MemoryRefreshTokenStore {
    fn add_token(&self, user_id: &str, token: &str) -> Result<(), RefreshTokenError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| RefreshTokenError::StorageError {
                context: "Cannot access refresh token store: mutex lock poisoned".to_string(),
                source: None,
            })?;
        inner.insert(user_id.to_string(), token.to_string());
        Ok(())
    }

    fn remove_token(&self, user_id: &str) -> Result<(), RefreshTokenError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| RefreshTokenError::StorageError {
                context: "Cannot access refresh token store: mutex lock poisoned".to_string(),
                source: None,
            })?;

        if inner.remove(user_id).is_some() {
            Ok(())
        } else {
            Err(RefreshTokenError::NotFoundError(format!(
                "User id {} not found.",
                user_id
            )))
        }
    }

    fn update_token(&self, user_id: &str, token: &str) -> Result<(), RefreshTokenError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| RefreshTokenError::StorageError {
                context: "Cannot access refresh token store: mutex lock poisoned".to_string(),
                source: None,
            })?;

        if inner.contains_key(user_id) {
            inner.insert(user_id.to_string(), token.to_string());
            Ok(())
        } else {
            Err(RefreshTokenError::NotFoundError(format!(
                "User id {} not found.",
                user_id
            )))
        }
    }

    fn fetch_token(&self, user_id: &str) -> Result<String, RefreshTokenError> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| RefreshTokenError::StorageError {
                context: "Cannot access refresh token store: mutex lock poisoned".to_string(),
                source: None,
            })?;

        if let Some(token) = inner.get(user_id) {
            Ok(token.to_string())
        } else {
            Err(RefreshTokenError::NotFoundError(format!(
                "User id {} not found.",
                user_id
            )))
        }
    }
}
