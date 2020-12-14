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

//! Provides structures for the REST resources.

#[cfg(feature = "biome-credentials")]
pub(in crate::biome::rest_api) mod authorize;
#[cfg(feature = "biome-credentials")]
pub(in crate::biome::rest_api) mod credentials;
#[cfg(feature = "biome-key-management")]
pub(in crate::biome::rest_api) mod key_management;
#[cfg(feature = "biome-credentials")]
pub(in crate::biome::rest_api) mod token;
#[cfg(all(feature = "biome-key-management", feature = "biome-credentials"))]
pub(in crate::biome::rest_api) mod user;

/// Represents a user of a splinter application
#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct User {
    id: String,
}

impl User {
    /// Creates a new User
    ///
    /// # Arguments
    ///
    /// * `user_id`: unique identifier for the user being created
    pub fn new(user_id: &str) -> Self {
        User {
            id: user_id.to_string(),
        }
    }

    /// Returns the user's id.
    pub fn id(&self) -> &str {
        &self.id
    }
}
