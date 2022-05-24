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

use crate::error::InvalidStateError;
use crate::rest_api::auth::authorization::rbac::store::Role;

/// Updates an existing role.
///
/// This builder only allows the updatable fields to be modified.
pub struct RoleUpdateBuilder {
    id: String,
    display_name: Option<String>,
    permissions: Vec<String>,
}

impl RoleUpdateBuilder {
    pub fn new(id: String) -> Self {
        Self {
            id,
            display_name: None,
            permissions: Vec::new(),
        }
    }
    /// Updates the display name for the updated role.
    pub fn with_display_name(mut self, display_name: String) -> Self {
        self.display_name = Some(display_name);
        self
    }

    /// Updates the permissions for the updated role.
    pub fn with_permissions(mut self, permissions: Vec<String>) -> Self {
        self.permissions = permissions;
        self
    }

    /// Builds the updated Role.
    ///
    /// # Errors
    ///
    /// Returns an [`InvalidStateError`] under the following conditions:
    /// * an empty display name was provided
    /// * empty permissions were provided
    pub fn build(self) -> Result<Role, InvalidStateError> {
        if self.permissions.is_empty() {
            return Err(InvalidStateError::with_message(
                "A role requires at least one permission".into(),
            ));
        }

        let display_name = self.display_name.ok_or_else(|| {
            InvalidStateError::with_message("A role requires a display_name field".into())
        })?;

        if display_name.is_empty() {
            return Err(InvalidStateError::with_message(
                "A role requires a non-empty display_name field".into(),
            ));
        }

        Ok(Role {
            id: self.id,
            display_name,
            permissions: self.permissions,
        })
    }
}
