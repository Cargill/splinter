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

//! Tools for identifying clients and users

#[cfg(feature = "biome-credentials")]
pub mod biome;
#[cfg(feature = "cylinder-jwt")]
pub mod cylinder;
#[cfg(feature = "oauth")]
pub mod oauth;

use crate::error::InternalError;

use super::AuthorizationHeader;

/// A REST API client's identity as determined by an [IdentityProvider]
#[derive(Debug, PartialEq)]
pub enum Identity {
    /// A custom identity
    Custom(String),
    #[cfg(feature = "cylinder-jwt")]
    /// A Cylinder public key
    Key(String),
    #[cfg(any(feature = "biome-credentials", feature = "oauth"))]
    /// A Biome user ID
    User(String),
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
    ///
    /// # Example
    ///
    ///```ignore
    ///  fn clone_box(&self) -> Box<dyn IdentityProvider> {
    ///     Box::new(self.clone())
    ///  }
    ///```
    fn clone_box(&self) -> Box<dyn IdentityProvider>;
}

impl Clone for Box<dyn IdentityProvider> {
    fn clone(&self) -> Box<dyn IdentityProvider> {
        self.clone_box()
    }
}
