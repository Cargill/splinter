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

//! Role-based permissions for public keys.
//!
//! Key permissions, accessed via the `KeyPermissionManager` interface, are queried through a simple
//! role-based access system.  The underlying implementation determines how those values are set
//! and modified.

mod error;
pub mod insecure;

pub use error::KeyPermissionError;

type KeyPermissionResult<T> = Result<T, KeyPermissionError>;

/// Manages role-based permissions associated with public keys.
///
/// The KeyPermissionManager provides an interface for providing details on whether or not a public
/// key has permissions to act in specific roles.
///
/// Note: the underlying implementation determines how those values are set and modified - these
/// operations are not exposed via this interface.
pub trait KeyPermissionManager: Send {
    /// Checks to see if a public key is permitted for the given role.
    ///
    /// # Errors
    ///
    /// Returns a `KeyPermissionError` if the underling implementation encountered an error while
    /// checking the permissions.
    fn is_permitted(&self, public_key: &[u8], role: &str) -> KeyPermissionResult<bool>;
}
