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
use crate::rbac::store::Assignment;
use crate::rbac::store::Identity;

/// Updates an existing assignment.
///
/// This builder only allows the updatable fields to be modified.
pub struct AssignmentUpdateBuilder {
    identity: Identity,
    roles: Vec<String>,
}

impl AssignmentUpdateBuilder {
    pub(super) fn new(identity: Identity) -> Self {
        Self {
            identity,
            roles: Vec::new(),
        }
    }
    /// Updates the assigned roles.
    pub fn with_roles(mut self, roles: Vec<String>) -> Self {
        self.roles = roles;
        self
    }

    /// Builds the updated assignment.
    ///
    /// # Errors
    ///
    /// Returns an [`InvalidStateError`] under the following conditions:
    /// * no roles were provided
    pub fn build(self) -> Result<Assignment, InvalidStateError> {
        if self.roles.is_empty() {
            return Err(InvalidStateError::with_message(
                "An assignment requires at least one role".into(),
            ));
        }

        Ok(Assignment {
            identity: self.identity,
            roles: self.roles,
        })
    }
}
