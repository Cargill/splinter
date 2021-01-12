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

#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub(in crate::biome) mod diesel;
pub mod error;
pub(in crate::biome) mod memory;

use crate::error::InvalidStateError;

use serde::{Deserialize, Serialize};

pub use error::UserProfileStoreError;

#[cfg(feature = "diesel")]
use self::diesel::models::NewProfileModel;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    user_id: String,
    name: Option<String>,
    given_name: Option<String>,
    family_name: Option<String>,
    email: Option<String>,
    picture: Option<String>,
}

impl Profile {
    /// Returns the user_id for the profile
    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    /// Returns the name for the profile
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Returns the given name for the profile
    pub fn given_name(&self) -> Option<&str> {
        self.given_name.as_deref()
    }

    /// Returns the family name for the profile
    pub fn family_name(&self) -> Option<&str> {
        self.family_name.as_deref()
    }

    /// Returns the email for the profile
    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    /// Returns the picture for the profile
    pub fn picture(&self) -> Option<&str> {
        self.picture.as_deref()
    }
}

/// Builder for profile.
#[derive(Default)]
pub struct ProfileBuilder {
    user_id: Option<String>,
    name: Option<String>,
    given_name: Option<String>,
    family_name: Option<String>,
    email: Option<String>,
    picture: Option<String>,
}

impl ProfileBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the user id for the profile
    pub fn with_user_id(mut self, user_id: String) -> ProfileBuilder {
        self.user_id = Some(user_id);
        self
    }

    /// Sets the name for the profile
    pub fn with_name(mut self, name: Option<String>) -> ProfileBuilder {
        self.name = name;
        self
    }

    /// Sets the given name for the profile
    pub fn with_given_name(mut self, given_name: Option<String>) -> ProfileBuilder {
        self.given_name = given_name;
        self
    }

    /// Sets the family name for the profile
    pub fn with_family_name(mut self, family_name: Option<String>) -> ProfileBuilder {
        self.family_name = family_name;
        self
    }

    /// Sets the email for the profile
    pub fn with_email(mut self, email: Option<String>) -> ProfileBuilder {
        self.email = email;
        self
    }

    /// Sets the picture for the profile
    pub fn with_picture(mut self, picture: Option<String>) -> ProfileBuilder {
        self.picture = picture;
        self
    }

    /// Builds the profile
    pub fn build(self) -> Result<Profile, InvalidStateError> {
        Ok(Profile {
            user_id: self.user_id.ok_or_else(|| {
                InvalidStateError::with_message("A user id is required to build a Profile".into())
            })?,
            name: self.name,
            given_name: self.given_name,
            family_name: self.family_name,
            email: self.email,
            picture: self.picture,
        })
    }
}

/// Defines methods for CRUD operations and fetching a user’s
/// profile without defining a storage strategy
pub trait UserProfileStore: Sync + Send {
    /// Adds a profile to the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `profile` - The profile to be added
    ///
    /// # Errors
    ///
    /// Returns a UserProfileStoreError if the implementation cannot add a new
    /// profile.
    fn add_profile(&self, profile: Profile) -> Result<(), UserProfileStoreError>;

    /// Replaces a profile for a user in the underlying storage with a new profile.
    ///
    /// #Arguments
    ///
    ///  * `user_id` - The unique identifier of the user the profile belongs to
    ///  * `name` - The updated name for the user profile
    ///  * `given_name` - The updated given name for the user profile
    ///  * `family_name` - The updated family name for the user profile
    ///  * `email` - The updated email for the user profile
    ///  * `picture` - The updated picture for the user profile
    ///
    /// # Errors
    ///
    /// Returns a UserProfileStoreError if the implementation cannot update profile
    /// or if the specified profile does not exist.
    fn update_profile(&self, profile: Profile) -> Result<(), UserProfileStoreError>;

    /// Removes a profile from the underlying storage.
    ///
    /// # Arguments
    ///
    ///  * `user_id`: The unique identifier of the user the profile belongs to
    ///
    /// # Errors
    ///
    /// Returns a UserProfileStoreError if the implementation cannot remove the
    /// profile or if a profile with the specified `user_id` does not exist.
    fn remove_profile(&self, user_id: &str) -> Result<(), UserProfileStoreError>;

    /// Fetches a profile from the underlying storage.
    ///
    /// # Arguments
    ///
    ///  * `user_id` - The unique identifier of the user the profile belongs to
    ///
    /// # Errors
    ///
    /// Returns a UserProfileStoreError if the implementation cannot retrieve the
    /// profile or if a profile with the specified `user_id` does not exist.
    fn get_profile(&self, user_id: &str) -> Result<Profile, UserProfileStoreError>;

    /// List all profiles from the underlying storage.
    ///
    /// # Errors
    ///
    /// Returns a UserProfileStoreError if implementation cannot fetch the stored
    /// profiles.
    fn list_profiles(&self) -> Result<Option<Vec<Profile>>, UserProfileStoreError>;

    /// Clone into a boxed, dynamically dispatched store
    fn clone_box(&self) -> Box<dyn UserProfileStore>;
}

impl Clone for Box<dyn UserProfileStore> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

impl<PS> UserProfileStore for Box<PS>
where
    PS: UserProfileStore + ?Sized,
{
    fn add_profile(&self, profile: Profile) -> Result<(), UserProfileStoreError> {
        (**self).add_profile(profile)
    }

    fn update_profile(&self, profile: Profile) -> Result<(), UserProfileStoreError> {
        (**self).update_profile(profile)
    }

    fn remove_profile(&self, user_id: &str) -> Result<(), UserProfileStoreError> {
        (**self).remove_profile(user_id)
    }

    fn get_profile(&self, user_id: &str) -> Result<Profile, UserProfileStoreError> {
        (**self).get_profile(user_id)
    }

    fn list_profiles(&self) -> Result<Option<Vec<Profile>>, UserProfileStoreError> {
        (**self).list_profiles()
    }

    fn clone_box(&self) -> Box<dyn UserProfileStore> {
        (**self).clone_box()
    }
}

#[cfg(feature = "diesel")]
impl Into<NewProfileModel> for Profile {
    fn into(self) -> NewProfileModel {
        NewProfileModel {
            user_id: self.user_id,
            name: self.name,
            given_name: self.given_name,
            family_name: self.family_name,
            email: self.email,
            picture: self.picture,
        }
    }
}
