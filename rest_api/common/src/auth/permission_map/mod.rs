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

#[cfg(test)]
mod method;
mod path_component;
mod request_definition;

use std::borrow::Borrow;

use super::Permission;

#[cfg(test)]
pub use method::Method;

use path_component::PathComponent;
use request_definition::RequestDefinition;

/// A map used to correlate requests with the permissions that guard them.
pub struct PermissionMap<M> {
    internal: Vec<(RequestDefinition<M>, Permission)>,
}

impl<M: PartialEq + Clone> Default for PermissionMap<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: PartialEq + Clone> PermissionMap<M> {
    /// Creates a new permission map
    pub fn new() -> Self {
        Self {
            internal: Vec::new(),
        }
    }

    /// Gets a list of all permissions.
    pub fn permissions(&self) -> impl Iterator<Item = Permission> + '_ {
        self.internal.iter().map(|(_, perm)| *perm)
    }

    /// Takes the contents of another `PermissionMap` and merges them into itself. This consumes the
    /// contents of the other map.
    pub fn append(&mut self, other: &mut PermissionMap<M>) {
        self.internal.append(&mut other.internal)
    }

    /// Sets the permission for the given (method, endpoint) pair. The endpoint may contain path
    /// variables surrounded by `{}`.
    pub fn add_permission(&mut self, method: M, endpoint: &str, permission: Permission) {
        self.internal
            .push((RequestDefinition::new(method, endpoint), permission));
    }

    /// Gets the permission for a request. This will attempt to match the method and endpoint to a
    /// known (method, endpoint) pair, considering path variables of known endpoints.
    pub fn get_permission<O>(&self, method: &O, endpoint: &str) -> Option<&Permission>
    where
        O: Borrow<M>,
    {
        self.internal
            .iter()
            .find(|(req, _)| req.matches(method.borrow(), endpoint))
            .map(|(_, perm)| perm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that a `PathComponent` is correctly parsed
    #[test]
    fn path_component_parse() {
        assert!(PathComponent::Text("".into()) == "");
        assert!(PathComponent::Text("test".into()) == "test");
        assert!(PathComponent::Variable == "{test}");
    }

    /// Verifies that a `PathComponent` can be correctly compared with a `&str`
    #[test]
    fn path_component_str_comparison() {
        assert!(PathComponent::Variable == "test1");
        assert!(PathComponent::Variable == "test2");
        assert!(PathComponent::Text("test1".into()) == "test1");
        assert!(PathComponent::Text("test1".into()) != "test2");
    }

    /// Verifies that the `RequestDefinition` struct works correctly for matching requests
    #[test]
    fn request_definition() {
        let definition = RequestDefinition::new(Method::Get, "/test/endpoint");
        assert!(definition.matches(&Method::Get, "/test/endpoint"));
        assert!(!definition.matches(&Method::Put, "/test/endpoint"));
        assert!(!definition.matches(&Method::Get, "/test/other"));
        assert!(!definition.matches(&Method::Get, "/test"));
        assert!(!definition.matches(&Method::Get, "/test/endpoint/test"));

        let definition = RequestDefinition::new(Method::Get, "/test/endpoint/{variable}");
        assert!(definition.matches(&Method::Get, "/test/endpoint/val1"));
        assert!(definition.matches(&Method::Get, "/test/endpoint/val2"));
        assert!(!definition.matches(&Method::Put, "/test/endpoint/val1"));

        let definition = RequestDefinition::new(Method::Get, "/");
        assert!(definition.matches(&Method::Get, "/"));
    }

    /// Verifies that the `PermissionMap` works correctly
    #[test]
    fn permission_map() {
        let perm1 = Permission::Check {
            permission_id: "perm1",
            permission_display_name: "",
            permission_description: "",
        };
        let perm2 = Permission::Check {
            permission_id: "perm2",
            permission_display_name: "",
            permission_description: "",
        };

        let mut map = PermissionMap::new();
        assert!(map.internal.is_empty());

        map.add_permission(Method::Get, "/test/endpoint", perm1);
        assert_eq!(map.internal.len(), 1);
        assert_eq!(
            map.get_permission(&Method::Get, "/test/endpoint"),
            Some(&perm1)
        );
        assert_eq!(map.get_permission(&Method::Put, "/test/endpoint"), None);
        assert_eq!(map.get_permission(&Method::Get, "/test/other"), None);

        let mut other_map = PermissionMap::new();
        other_map.add_permission(Method::Put, "/test/endpoint/{variable}", perm2);
        map.append(&mut other_map);
        assert_eq!(map.internal.len(), 2);
        assert_eq!(
            map.get_permission(&Method::Get, "/test/endpoint"),
            Some(&perm1)
        );
        assert_eq!(
            map.get_permission(&Method::Put, "/test/endpoint/test1"),
            Some(&perm2)
        );
        assert_eq!(
            map.get_permission(&Method::Put, "/test/endpoint/test2"),
            Some(&perm2)
        );
        assert_eq!(
            map.get_permission(&Method::Get, "/test/endpoint/test1"),
            None
        );
    }
}
