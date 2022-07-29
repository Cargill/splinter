// Copyright 2018-2022 Cargill Incorporated
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

//! Tools for identifying clients and users

#[cfg(feature = "biome-credentials")]
pub mod biome;
#[cfg(feature = "cylinder-jwt")]
pub mod cylinder;
#[cfg(feature = "oauth")]
pub mod oauth;

use splinter::error::InternalError;

use crate::auth::AuthorizationHeader;

/// A REST API client's identity as determined by an [IdentityProvider]
#[derive(Debug, PartialEq)]
pub enum Identity {
    /// A custom identity
    Custom(String),
    /// A public key
    Key(String),
    /// A Biome user ID
    User(String),
}

#[cfg(feature = "rbac")]
impl From<&Identity> for Option<splinter::rbac::store::Identity> {
    fn from(identity: &Identity) -> Self {
        match identity {
            // RoleBasedAuthorization does not currently support custom identities
            Identity::Custom(_) => None,
            Identity::Key(key) => Some(splinter::rbac::store::Identity::Key(key.to_string())),
            Identity::User(user_id) => {
                Some(splinter::rbac::store::Identity::User(user_id.to_string()))
            }
        }
    }
}

/// A service that fetches identities from a backing provider
pub trait IdentityProvider: Send + Sync {
    /// Attempts to get the identity that corresponds to the given authorization header. This method
    /// will return `Ok(None)` if the identity provider was not able to resolve the authorization
    /// to an identity.
    fn get_identity(
        &self,
        authorization: &AuthorizationHeader,
    ) -> Result<Option<Identity>, InternalError>;

    /// Clone implementation for `IdentityProvider`. The implementation of the `Clone` trait for
    /// `Box<dyn IdentityProvider>` calls this method.
    fn clone_box(&self) -> Box<dyn IdentityProvider>;
}

impl Clone for Box<dyn IdentityProvider> {
    fn clone(&self) -> Box<dyn IdentityProvider> {
        self.clone_box()
    }
}
