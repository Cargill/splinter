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

//! Defines a representation of an OAuth user and provides an API to manage them.
//!
//! The OAuth user can be considered an extension of the base Biome user.

#[cfg(feature = "diesel")]
pub(in crate::biome) mod diesel;
mod error;
pub(in crate::biome) mod memory;

pub use error::InvalidStateError;
pub use error::OAuthUserStoreError;

/// The set of supported OAuth providers.
#[derive(Clone, Debug, PartialEq)]
pub enum OAuthProvider {
    Github,
}

/// A user defined by an OAuth Provider.
///
/// This user is connected to a Biome User, via a user ID.
#[derive(Clone)]
pub struct OAuthUser {
    user_id: String,
    provider_user_ref: String,

    access_token: String,
    refresh_token: Option<String>,
    provider: OAuthProvider,
}

impl OAuthUser {
    /// Return the user ID associated with this OAuth user
    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    /// Return the user's provider user reference.
    ///
    /// This references the identity value of the user in the provider's system.
    pub fn provider_user_ref(&self) -> &str {
        &self.provider_user_ref
    }

    /// Return the user's current access token.
    pub fn access_token(&self) -> &str {
        &self.access_token
    }

    /// Return the user's current refresh token, if one is available.
    pub fn refresh_token(&self) -> Option<&str> {
        self.refresh_token.as_deref()
    }

    /// Return the OAuth provider used
    pub fn provider(&self) -> &OAuthProvider {
        &self.provider
    }

    /// Convert this OAuthUser into an update builder.
    pub fn into_update_builder(self) -> OAuthUserUpdateBuilder {
        let Self {
            user_id,
            provider_user_ref,
            access_token,
            refresh_token,
            provider,
        } = self;
        OAuthUserUpdateBuilder {
            user_id,
            provider_user_ref,
            access_token,
            refresh_token,
            provider,
        }
    }
}

/// Builder for OAuthUser structs
#[derive(Default)]
pub struct OAuthUserBuilder {
    user_id: Option<String>,
    provider_user_ref: Option<String>,

    access_token: Option<String>,
    refresh_token: Option<String>,
    provider: Option<OAuthProvider>,
}

impl OAuthUserBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the Biome ID for this OAuth user.
    pub fn with_user_id(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);

        self
    }

    /// Set the user identity, as defined by the OAuth provider.
    pub fn with_provider_user_ref(mut self, provider_user_ref: String) -> Self {
        self.provider_user_ref = Some(provider_user_ref);

        self
    }

    /// Set the OAuth access token.
    pub fn with_access_token(mut self, access_token: String) -> Self {
        self.access_token = Some(access_token);

        self
    }

    /// Set the OAuth refresh token.
    ///
    /// This field is optional when constructing the final struct.
    pub fn with_refresh_token(mut self, refresh_token: String) -> Self {
        self.refresh_token = Some(refresh_token);

        self
    }

    /// Set the OAuth provider used to create this user.
    pub fn with_provider(mut self, provider: OAuthProvider) -> Self {
        self.provider = Some(provider);

        self
    }

    /// Build an OAuthUser
    ///
    /// # Errors
    ///
    /// Returns an `InvalidStateError` if there are required fields missing.
    pub fn build(self) -> Result<OAuthUser, InvalidStateError> {
        Ok(OAuthUser {
            user_id: self.user_id.ok_or_else(|| {
                InvalidStateError("A user ID is required to successfully build an OAuthUser".into())
            })?,
            provider_user_ref: self.provider_user_ref.ok_or_else(|| {
                InvalidStateError(
                    "A provider user identity is required to successfully build an OAuthUser"
                        .into(),
                )
            })?,
            access_token: self.access_token.ok_or_else(|| {
                InvalidStateError(
                    "An access token is required to successfully build an OAuthUser".into(),
                )
            })?,
            refresh_token: self.refresh_token,
            provider: self.provider.ok_or_else(|| {
                InvalidStateError(
                    "A provider is required to successfully build an OAuthUser".into(),
                )
            })?,
        })
    }
}

/// Builds an updated `OAuthUser` struct.
///
/// This builder only allows changes to the fields on an OAuthUser that may be
/// updated.
pub struct OAuthUserUpdateBuilder {
    // "immutable" items
    user_id: String,
    provider_user_ref: String,
    provider: OAuthProvider,

    // "mutable" items
    access_token: String,
    refresh_token: Option<String>,
}

impl OAuthUserUpdateBuilder {
    /// Set the OAuth access token.
    pub fn with_access_token(mut self, access_token: String) -> Self {
        self.access_token = access_token;

        self
    }

    /// Set the OAuth refresh token.
    ///
    /// This field is optional when constructing the final struct.
    pub fn with_refresh_token(mut self, refresh_token: String) -> Self {
        self.refresh_token = Some(refresh_token);

        self
    }

    /// Builds the updated OAuthUser.
    pub fn build(self) -> Result<OAuthUser, InvalidStateError> {
        let Self {
            user_id,
            provider_user_ref,
            access_token,
            refresh_token,
            provider,
        } = self;
        Ok(OAuthUser {
            user_id,
            provider_user_ref,
            access_token,
            refresh_token,
            provider,
        })
    }
}

/// Defines methods for CRUD operations and fetching OAuth user information.
pub trait OAuthUserStore {
    /// Add an OAuthUser to the store.
    ///
    /// # Errors
    ///
    /// Returns a ConstraintViolation if either there already is a user ID associated
    /// with another provider identity, or the provider identity has already been
    /// associated with a user ID.
    fn add_oauth_user(&self, oauth_user: OAuthUser) -> Result<(), OAuthUserStoreError>;

    /// Updates an OAuthUser to the store.
    ///
    /// # Errors
    ///
    /// Returns a ConstraintViolation if the OAuthUser associated with the user ID provided doesn't
    /// exist.
    fn update_oauth_user(&self, oauth_user: OAuthUser) -> Result<(), OAuthUserStoreError>;

    /// Returns the stored OAuth user based on the provider_user_ref from the OAuth provider.
    fn get_by_provider_user_ref(
        &self,
        provider_user_ref: &str,
    ) -> Result<Option<OAuthUser>, OAuthUserStoreError>;

    /// Returns the stored OAuth user based on the biome user ID.
    fn get_by_user_id(&self, user_id: &str) -> Result<Option<OAuthUser>, OAuthUserStoreError>;
}
