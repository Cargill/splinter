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

#[cfg(not(feature = "actix-web-4"))]
use crate::rest_api::Method as SuperMethod;
#[cfg(feature = "actix-web-4")]
use actix_web_4::http::Method as ActixMethod;

use super::Permission;

/// A map used to correlate requests with the permissions that guard them.
#[derive(Default)]
pub(in crate::rest_api) struct PermissionMap {
    internal: Vec<(RequestDefinition, Permission)>,
}

impl PermissionMap {
    /// Creates a new permission map
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets a list of all permissions.
    pub fn permissions(&self) -> impl Iterator<Item = Permission> + '_ {
        self.internal.iter().map(|(_, perm)| *perm)
    }

    /// Sets the permission for the given (method, endpoint) pair. The endpoint may contain path
    /// variables surrounded by `{}`.
    pub fn add_permission<M>(&mut self, method: M, endpoint: &str, permission: Permission)
    where
        M: Into<Method>,
    {
        self.internal
            .push((RequestDefinition::new(method.into(), endpoint), permission));
    }

    /// Gets the permission for a request. This will attempt to match the method and endpoint to a
    /// known (method, endpoint) pair, considering path variables of known endpoints.
    pub fn get_permission<M>(&self, method: M, endpoint: &str) -> Option<&Permission>
    where
        M: Into<Method> + Clone + Copy,
    {
        self.internal
            .iter()
            .find(|(req, _)| req.matches(&method.into(), endpoint))
            .map(|(_, perm)| perm)
    }

    /// Takes the contents of another `PermissionMap` and merges them into itself. This consumes the
    /// contents of the other map.
    pub fn append(&mut self, other: &mut PermissionMap) {
        self.internal.append(&mut other.internal)
    }
}

#[derive(PartialEq, Clone)]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
    Connect,
    Trace,
    Extension(String),
}

#[cfg(feature = "actix-web-4")]
impl From<ActixMethod> for Method {
    fn from(source: ActixMethod) -> Self {
        match source.as_str() {
            "OPTIONS" => Method::Options,
            "GET" => Method::Get,
            "POST" => Method::Post,
            "PUT" => Method::Put,
            "DELETE" => Method::Delete,
            "HEAD" => Method::Head,
            "TRACE" => Method::Trace,
            "CONNECT" => Method::Connect,
            "PATCH" => Method::Patch,
            other => Method::Extension(other.to_string()),
        }
    }
}

/*
#[cfg(not(feature = "actix-web-4"))]
impl From<SuperMethod> for Method {
    fn from(source: SuperMethod) -> Self {
        Self {}
    }
}

 */
#[cfg(feature = "actix-web-4")]
impl From<&reqwest::Method> for Method {
    fn from(source: &reqwest::Method) -> Self {
        match source.as_str() {
            "OPTIONS" => Method::Options,
            "GET" => Method::Get,
            "POST" => Method::Post,
            "PUT" => Method::Put,
            "DELETE" => Method::Delete,
            "HEAD" => Method::Head,
            "TRACE" => Method::Trace,
            "CONNECT" => Method::Connect,
            "PATCH" => Method::Patch,
            other => Method::Extension(other.to_string()),
        }
    }
}

impl From<&crate::rest_api::actix_web_1::Method> for Method {
    fn from(source: &crate::rest_api::actix_web_1::Method) -> Self {
        match source {
            crate::rest_api::actix_web_1::Method::Get => Method::Get,
            crate::rest_api::actix_web_1::Method::Post => Method::Post,
            crate::rest_api::actix_web_1::Method::Put => Method::Put,
            crate::rest_api::actix_web_1::Method::Patch => Method::Patch,
            crate::rest_api::actix_web_1::Method::Delete => Method::Delete,
            crate::rest_api::actix_web_1::Method::Head => Method::Head,
        }
    }
}
impl From<crate::rest_api::actix_web_1::Method> for Method {
    fn from(source: crate::rest_api::actix_web_1::Method) -> Self {
        (&source).into()
    }
}

/// A (method, endpoint) definition that will be used to match requests
struct RequestDefinition {
    method: Method,
    path: Vec<PathComponent>,
}

impl RequestDefinition {
    /// Creates a new request definition
    pub fn new(method: Method, endpoint: &str) -> Self {
        let path = endpoint
            .strip_prefix('/')
            .unwrap_or(endpoint)
            .split('/')
            .map(PathComponent::from)
            .collect();

        Self { method, path }
    }

    /// Checks if the given request matches this definition, considering any variable path
    /// components.
    pub fn matches(&self, method: &Method, endpoint: &str) -> bool {
        let components = endpoint
            .strip_prefix('/')
            .unwrap_or(endpoint)
            .split('/')
            .collect::<Vec<_>>();

        method == &self.method
            && self.path.len() == components.len()
            && components.iter().enumerate().all(|(idx, component)| {
                self.path
                    .get(idx)
                    .map(|path_component| path_component == component)
                    .unwrap_or(false)
            })
    }
}

/// A component of an endpoint path
#[derive(PartialEq)]
enum PathComponent {
    /// A standard path component where matching is done on the internal string
    Text(String),
    /// A variable path component that matches any string
    Variable,
}

impl From<&str> for PathComponent {
    fn from(component: &str) -> Self {
        if component.starts_with('{') && component.ends_with('}') {
            PathComponent::Variable
        } else {
            PathComponent::Text(component.into())
        }
    }
}

impl PartialEq<&str> for PathComponent {
    fn eq(&self, other: &&str) -> bool {
        match self {
            PathComponent::Variable => true,
            PathComponent::Text(component) => other == component,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that a `PathComponent` is correctly parsed
    #[test]
    fn path_component_parse() {
        assert!(PathComponent::from("") == PathComponent::Text("".into()));
        assert!(PathComponent::from("test") == PathComponent::Text("test".into()));
        assert!(PathComponent::from("{test}") == PathComponent::Variable);
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
