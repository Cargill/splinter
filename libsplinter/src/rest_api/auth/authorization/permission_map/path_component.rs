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

/// A component of an endpoint path
#[derive(PartialEq, Eq)]
pub enum PathComponent {
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
