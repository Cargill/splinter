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

//! A memory-backed implementation of the [UserProfileStore]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::error::{InternalError, InvalidArgumentError, InvalidStateError};

use super::{error::UserProfileStoreError, Profile, ProfileBuilder, UserProfileStore};

#[derive(Default, Clone)]
pub struct MemoryUserProfileStore {
    inner: Arc<Mutex<HashMap<String, Profile>>>,
}

impl MemoryUserProfileStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl UserProfileStore for MemoryUserProfileStore {
    fn add_profile(&self, profile: Profile) -> Result<(), UserProfileStoreError> {
        let mut inner = self.inner.lock().map_err(|_| {
            UserProfileStoreError::Internal(InternalError::with_message(
                "Cannot access user profile store: mutex lock poisoned".to_string(),
            ))
        })?;

        inner.insert(profile.user_id.clone(), profile);
        Ok(())
    }

    fn update_profile(&self, profile: Profile) -> Result<(), UserProfileStoreError> {
        let mut inner = self.inner.lock().map_err(|_| {
            UserProfileStoreError::Internal(InternalError::with_message(
                "Cannot access user profile store: mutex lock poisoned".to_string(),
            ))
        })?;
        if inner.contains_key(&profile.user_id) {
            let new_profile = ProfileBuilder::default()
                .with_user_id(profile.user_id.clone())
                .with_name(profile.name)
                .with_given_name(profile.given_name)
                .with_family_name(profile.family_name)
                .with_email(profile.email)
                .build()
                .map_err(|_| {
                    UserProfileStoreError::Internal(InternalError::with_message(
                        "Failed to build profile with updated details".to_string(),
                    ))
                })?;
            inner.insert(profile.user_id, new_profile);
            Ok(())
        } else {
            Err(UserProfileStoreError::InvalidArgument(
                InvalidArgumentError::new(
                    "user_id".to_string(),
                    "A profile for the given user_id does not exist".to_string(),
                ),
            ))
        }
    }

    fn remove_profile(&self, user_id: &str) -> Result<(), UserProfileStoreError> {
        let mut inner = self.inner.lock().map_err(|_| {
            UserProfileStoreError::Internal(InternalError::with_message(
                "Cannot access user profile store: mutex lock poisoned".to_string(),
            ))
        })?;
        if inner.remove(user_id).is_some() {
            Ok(())
        } else {
            Err(UserProfileStoreError::InvalidState(
                InvalidStateError::with_message(
                    "A profile with the given user id does not exist".to_string(),
                ),
            ))
        }
    }

    fn get_profile(&self, user_id: &str) -> Result<Profile, UserProfileStoreError> {
        let inner = self.inner.lock().map_err(|_| {
            UserProfileStoreError::Internal(InternalError::with_message(
                "Cannot access user profile store: mutex lock poisoned".to_string(),
            ))
        })?;
        if let Some(profile) = inner.get(user_id) {
            Ok(profile.clone())
        } else {
            Err(UserProfileStoreError::InvalidArgument(
                InvalidArgumentError::new(
                    "user_id".to_string(),
                    "A profile for the given user_id does not exist".to_string(),
                ),
            ))
        }
    }

    fn list_profiles(&self) -> Result<Option<Vec<Profile>>, UserProfileStoreError> {
        let inner = self.inner.lock().map_err(|_| {
            UserProfileStoreError::Internal(InternalError::with_message(
                "Cannot access user profile store: mutex lock poisoned".to_string(),
            ))
        })?;
        Ok(Some(
            inner.iter().map(|(_, profile)| profile.clone()).collect(),
        ))
    }

    fn clone_box(&self) -> Box<dyn UserProfileStore> {
        Box::new(self.clone())
    }
}
