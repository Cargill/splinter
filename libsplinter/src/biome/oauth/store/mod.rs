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

//! Defines a representation of OAuth users and their sessions with an API to manage them.
//!
//! This store serves two purposes:
//!
//! * It provides a correlation between an OAuth subject identifier and a Biome user ID
//! * It stores tokens and other data for an OAuth user's sessions

#[cfg(any(feature = "biome-oauth-user-store-postgres", feature = "sqlite"))]
pub(in crate::biome) mod diesel;
mod error;
pub(in crate::biome) mod memory;

use std::time::SystemTime;

use uuid::Uuid;

use crate::error::InvalidStateError;

pub use error::OAuthUserSessionStoreError;

/// This is the UUID namespace for Biome user IDs generated for users that login with OAuth. This
/// will prevent collisions with Biome user IDs generated for users that register with Biome
/// credentials. The `u128` was calculated by creating a v5 UUID with the nil namespace and the
/// name `b"biome oauth"`.
const UUID_NAMESPACE: Uuid = Uuid::from_u128(187643141867173602676740887132833008173);

/// Correlation between an OAuth user (subject) and a Biome user ID
#[derive(Clone)]
pub struct OAuthUser {
    subject: String,
    user_id: String,
}

impl OAuthUser {
    /// Creates a new subject/user pair with a new generated Biome user ID
    ///
    /// This constructor should only be used by implementations of the [OAuthUserSessionStore] for
    /// creating a new user.
    pub fn new(subject: String) -> Self {
        Self {
            subject,
            user_id: Uuid::new_v5(&UUID_NAMESPACE, Uuid::new_v4().as_bytes()).to_string(),
        }
    }

    /// Creates a new subject/user pair with an existing Biome user ID
    ///
    /// This constructor should only be used by implementations of the [OAuthUserSessionStore] for
    /// returning an existing user.
    pub fn new_with_id(subject: String, user_id: String) -> Self {
        Self { subject, user_id }
    }

    /// Returns the user's subject identifier
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// Returns the Biome user ID
    pub fn user_id(&self) -> &str {
        &self.user_id
    }
}

/// Data for an OAuth user's session that's in an [OAuthUserSessionStore]
#[derive(Clone)]
pub struct OAuthUserSession {
    splinter_access_token: String,
    user: OAuthUser,
    oauth_access_token: String,
    oauth_refresh_token: Option<String>,
    last_authenticated: SystemTime,
}

impl OAuthUserSession {
    /// Returns the Splinter access token for this session. This token is sent by the client and
    /// verified by the Splinter REST API.
    pub fn splinter_access_token(&self) -> &str {
        &self.splinter_access_token
    }

    /// Returns the user this session is for
    pub fn user(&self) -> &OAuthUser {
        &self.user
    }

    /// Returns the OAuth access token associated with this session. This token may be used to
    /// reauthenticate the user with the OAuth provider.
    pub fn oauth_access_token(&self) -> &str {
        &self.oauth_access_token
    }

    /// Returns the OAuth refresh token associated with this session if it exists. This token may be
    /// used to get a new access token from the OAuth provider.
    pub fn oauth_refresh_token(&self) -> Option<&str> {
        self.oauth_refresh_token.as_deref()
    }

    /// Returns the time at which the user was last authenticated with the OAuth provider for this
    /// session. This may be used to determine when the user needs to be reauthenticated for the
    /// session. This field is only set by the store; when the session data is returned by the
    /// store, this field will always be set.
    pub fn last_authenticated(&self) -> SystemTime {
        self.last_authenticated
    }

    /// Converts the session data into an update builder
    pub fn into_update_builder(self) -> InsertableOAuthUserSessionUpdateBuilder {
        InsertableOAuthUserSessionUpdateBuilder {
            splinter_access_token: self.splinter_access_token,
            subject: self.user.subject,
            oauth_access_token: self.oauth_access_token,
            oauth_refresh_token: self.oauth_refresh_token,
        }
    }
}

/// Builds a new [OAuthUserSession]
///
/// This builder should only be used by implementations of the [OAuthUserSessionStore] for creating
/// session data to return.
#[derive(Default)]
pub struct OAuthUserSessionBuilder {
    splinter_access_token: Option<String>,
    user: Option<OAuthUser>,
    oauth_access_token: Option<String>,
    oauth_refresh_token: Option<String>,
    last_authenticated: Option<SystemTime>,
}

impl OAuthUserSessionBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the Splinter access token for this session
    pub fn with_splinter_access_token(mut self, splinter_access_token: String) -> Self {
        self.splinter_access_token = Some(splinter_access_token);
        self
    }

    /// Sets the user this session is for
    pub fn with_user(mut self, user: OAuthUser) -> Self {
        self.user = Some(user);
        self
    }

    /// Sets the OAuth access token for this session
    pub fn with_oauth_access_token(mut self, oauth_access_token: String) -> Self {
        self.oauth_access_token = Some(oauth_access_token);
        self
    }

    /// Sets the OAuth refresh token for this session
    pub fn with_oauth_refresh_token(mut self, oauth_refresh_token: Option<String>) -> Self {
        self.oauth_refresh_token = oauth_refresh_token;
        self
    }

    /// Sets the time at which the user was last authenticated for this session
    pub fn with_last_authenticated(mut self, last_authenticated: SystemTime) -> Self {
        self.last_authenticated = Some(last_authenticated);
        self
    }

    /// Builds the session
    pub fn build(self) -> Result<OAuthUserSession, InvalidStateError> {
        Ok(OAuthUserSession {
            splinter_access_token: self.splinter_access_token.ok_or_else(|| {
                InvalidStateError::with_message(
                    "A Splinter access token is required to build an OAuthUserSession".into(),
                )
            })?,
            user: self.user.ok_or_else(|| {
                InvalidStateError::with_message(
                    "A user is required to build an OAuthUserSession".into(),
                )
            })?,
            oauth_access_token: self.oauth_access_token.ok_or_else(|| {
                InvalidStateError::with_message(
                    "An OAuth access token is required to build an OAuthUserSession".into(),
                )
            })?,
            oauth_refresh_token: self.oauth_refresh_token,
            last_authenticated: self.last_authenticated.ok_or_else(|| {
                InvalidStateError::with_message(
                    "A 'last authenticated' time is required to build an OAuthUserSession".into(),
                )
            })?,
        })
    }
}

/// Data for an OAuth user's session that can be inserted into an [OAuthUserSessionStore]
///
/// Unlike [OAuthUserSession], this struct does not contain a `last_authenticated` timestamp or the
/// user's Biome user ID; this is because the timestamp and Biome user ID are always determined by
/// the store itself.
pub struct InsertableOAuthUserSession {
    splinter_access_token: String,
    subject: String,
    oauth_access_token: String,
    oauth_refresh_token: Option<String>,
}

impl InsertableOAuthUserSession {
    /// Returns the Splinter access token for this session
    pub fn splinter_access_token(&self) -> &str {
        &self.splinter_access_token
    }

    /// Returns the subject identifier of the user this session is for
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// Returns the OAuth access token associated with this session
    pub fn oauth_access_token(&self) -> &str {
        &self.oauth_access_token
    }

    /// Returns the OAuth refresh token associated with this session if it exists
    pub fn oauth_refresh_token(&self) -> Option<&str> {
        self.oauth_refresh_token.as_deref()
    }
}

/// Builds a new [InsertableOAuthUserSession]
#[derive(Default)]
pub struct InsertableOAuthUserSessionBuilder {
    splinter_access_token: Option<String>,
    subject: Option<String>,
    oauth_access_token: Option<String>,
    oauth_refresh_token: Option<String>,
}

impl InsertableOAuthUserSessionBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the Splinter access token for this session
    pub fn with_splinter_access_token(mut self, splinter_access_token: String) -> Self {
        self.splinter_access_token = Some(splinter_access_token);
        self
    }

    /// Sets the subject identifier of the user this session is for
    pub fn with_subject(mut self, subject: String) -> Self {
        self.subject = Some(subject);
        self
    }

    /// Sets the OAuth access token for this session
    pub fn with_oauth_access_token(mut self, oauth_access_token: String) -> Self {
        self.oauth_access_token = Some(oauth_access_token);
        self
    }

    /// Sets the OAuth refresh token for this session
    pub fn with_oauth_refresh_token(mut self, oauth_refresh_token: Option<String>) -> Self {
        self.oauth_refresh_token = oauth_refresh_token;
        self
    }

    /// Builds the insertable session
    pub fn build(self) -> Result<InsertableOAuthUserSession, InvalidStateError> {
        Ok(InsertableOAuthUserSession {
            splinter_access_token: self.splinter_access_token.ok_or_else(|| {
                InvalidStateError::with_message(
                    "A Splinter access token is required to build an InsertableOAuthUserSession"
                        .into(),
                )
            })?,
            subject: self.subject.ok_or_else(|| {
                InvalidStateError::with_message(
                    "A subject identifier is required to build an InsertableOAuthUserSession"
                        .into(),
                )
            })?,
            oauth_access_token: self.oauth_access_token.ok_or_else(|| {
                InvalidStateError::with_message(
                    "An OAuth access token is required to build an InsertableOAuthUserSession"
                        .into(),
                )
            })?,
            oauth_refresh_token: self.oauth_refresh_token,
        })
    }
}

/// Builds an updated [InsertableOAuthUserSession]
///
/// This builder only allows changes to the fields of a session that may be updated.
pub struct InsertableOAuthUserSessionUpdateBuilder {
    // Immutable items
    splinter_access_token: String,
    subject: String,
    // Mutable items
    oauth_access_token: String,
    oauth_refresh_token: Option<String>,
}

impl InsertableOAuthUserSessionUpdateBuilder {
    /// Sets the OAuth access token for this session
    pub fn with_oauth_access_token(mut self, oauth_access_token: String) -> Self {
        self.oauth_access_token = oauth_access_token;
        self
    }

    /// Sets the OAuth refresh token for this session
    pub fn with_oauth_refresh_token(mut self, oauth_refresh_token: Option<String>) -> Self {
        self.oauth_refresh_token = oauth_refresh_token;
        self
    }

    /// Builds the insertable session
    pub fn build(self) -> InsertableOAuthUserSession {
        InsertableOAuthUserSession {
            splinter_access_token: self.splinter_access_token,
            subject: self.subject,
            oauth_access_token: self.oauth_access_token,
            oauth_refresh_token: self.oauth_refresh_token,
        }
    }
}

/// Defines methods for CRUD operations on OAuth session data
pub trait OAuthUserSessionStore: Send + Sync {
    /// Adds an OAuth session
    ///
    /// The store will set the "last authenticated" value of the session to the current time. The
    /// store will also generate a new OAuth user entry if one does not already exist for the
    /// session's subject.
    ///
    /// # Errors
    ///
    /// Returns a `ConstraintViolation` error if a session with the given `splinter_access_token`
    /// already exists.
    fn add_session(
        &self,
        session: InsertableOAuthUserSession,
    ) -> Result<(), OAuthUserSessionStoreError>;

    /// Updates the OAuth access token and/or refresh token for a session
    ///
    /// The store will set the "last authenticated" value of the session to the current time.
    ///
    /// # Errors
    ///
    /// * Returns an `InvalidState` error if there is no session with the given
    ///   `splinter_access_token`
    /// * Returns a `InvalidArgument` error if any field other than `oauth_access_token` or
    ///   `oauth_refresh_token` have been changed.
    fn update_session(
        &self,
        session: InsertableOAuthUserSession,
    ) -> Result<(), OAuthUserSessionStoreError>;

    /// Removes an OAuth session based on the provided Splinter access token.
    ///
    /// # Errors
    ///
    /// Returns an `InvalidState` error if there is no session with the given
    /// `splinter_access_token`
    fn remove_session(&self, splinter_access_token: &str)
        -> Result<(), OAuthUserSessionStoreError>;

    /// Returns the OAuth session for the provided Splinter access token if it exists
    fn get_session(
        &self,
        splinter_access_token: &str,
    ) -> Result<Option<OAuthUserSession>, OAuthUserSessionStoreError>;

    /// Returns the correlation between the given OAuth subject identifier and a Biome user ID if it
    /// exists
    fn get_user(&self, subject: &str) -> Result<Option<OAuthUser>, OAuthUserSessionStoreError>;

    /// Clone into a boxed, dynamically dispatched store
    fn clone_box(&self) -> Box<dyn OAuthUserSessionStore>;
}

impl Clone for Box<dyn OAuthUserSessionStore> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
