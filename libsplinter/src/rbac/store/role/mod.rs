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

pub use builder::RoleBuilder;
pub use update_builder::RoleUpdateBuilder;

/// A Role is a named set of permissions.
#[derive(Clone)]
pub struct Role {
    id: String,
    display_name: String,
    permissions: Vec<String>,
}

impl Role {
    /// Returns the role's ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the role's display name.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Returns the role's permissions.
    pub fn permissions(&self) -> &[String] {
        &self.permissions
    }

    /// Convert this role back into a builder, in order to update its values.
    pub fn into_update_builder(self) -> RoleUpdateBuilder {
        RoleUpdateBuilder::new(self.id)
            .with_display_name(self.display_name)
            .with_permissions(self.permissions)
    }

    /// Converts this role into it's constituent parts.  These parts are in the tuple:
    /// `(id, display_name, permissions)`.
    pub fn into_parts(self) -> (String, String, Vec<String>) {
        (self.id, self.display_name, self.permissions)
    }
}
