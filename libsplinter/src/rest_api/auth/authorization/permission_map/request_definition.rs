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

use std::borrow::Borrow;

use super::PathComponent;

/// A (method, endpoint) definition that will be used to match requests
pub struct RequestDefinition<M> {
    method: M,
    path: Vec<PathComponent>,
}

impl<M: PartialEq + Clone> RequestDefinition<M> {
    /// Creates a new request definition
    pub fn new(method: M, endpoint: &str) -> Self {
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
    pub fn matches<O>(&self, method: &O, endpoint: &str) -> bool
    where
        M: Borrow<O>,
        O: PartialEq + Clone + PartialEq<M>,
    {
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
