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

mod builder;
mod update_builder;

use crate::rbac::store::Identity;

pub use builder::AssignmentBuilder;
pub use update_builder::AssignmentUpdateBuilder;

/// An assignment of roles to a particular identity.
#[derive(Clone)]
pub struct Assignment {
    identity: Identity,
    roles: Vec<String>,
}

impl Assignment {
    /// Returns the identity that has been assigned this set of roles.
    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    /// Returns the assigned roles IDs.
    pub fn roles(&self) -> &[String] {
        &self.roles
    }

    /// Convert this assignment back into a builder, in order to update its values.
    pub fn into_update_builder(self) -> AssignmentUpdateBuilder {
        let Assignment { identity, roles } = self;
        AssignmentUpdateBuilder::new(identity).with_roles(roles)
    }

    /// Converts this assignment into it's constituent parts.  These parts are in the tuple:
    /// `(identity, roles)`.
    pub fn into_parts(self) -> (Identity, Vec<String>) {
        (self.identity, self.roles)
    }

    #[cfg(feature = "diesel")]
    pub(super) fn new_unchecked(identity: Identity, roles: Vec<String>) -> Self {
        Self { identity, roles }
    }
}
