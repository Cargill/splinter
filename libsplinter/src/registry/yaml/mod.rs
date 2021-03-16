// Copyright 2018-2021 Cargill Incorporated
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

//! YAML file-backed registry implementations.

use std::collections::HashMap;
use std::convert::TryFrom;

mod local;
#[cfg(feature = "registry-remote")]
mod remote;

pub use crate::registry::error::InvalidNodeError;

use super::Node;

pub use local::LocalYamlRegistry;
#[cfg(feature = "registry-remote")]
pub use remote::{RemoteYamlRegistry, RemoteYamlShutdownHandle};

/// Yaml representation of a node in a registry.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct YamlNode {
    /// The Splinter identity of the node; must be non-empty and unique in the registry.
    identity: String,
    /// The endpoints the node can be reached at; at least one endpoint must be provided, and each
    /// endpoint must be non-empty and unique in the registry.
    endpoints: Vec<String>,
    /// A human-readable name for the node; must be non-empty.
    display_name: String,
    /// The list of public keys that are permitted to act on behalf of the node; at least one key
    /// must be provided, and each key must be non-empty.
    keys: Vec<String>,
    /// A map with node metadata.
    metadata: HashMap<String, String>,
}

impl YamlNode {
    /// The Splinter identity of the node;
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// The endpoints the node can be reached at
    pub fn endpoints(&self) -> &[String] {
        &self.endpoints
    }

    /// A human-readable name for the node
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// The list of public keys that are permitted to act on behalf of the node
    pub fn keys(&self) -> &[String] {
        &self.keys
    }

    /// A map with node metadata.
    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }
}

impl From<Node> for YamlNode {
    fn from(node: Node) -> Self {
        YamlNode {
            identity: node.identity().into(),
            endpoints: node.endpoints().into(),
            display_name: node.display_name().into(),
            keys: node.keys().into(),
            metadata: node.metadata().clone(),
        }
    }
}

impl TryFrom<YamlNode> for Node {
    type Error = InvalidNodeError;

    fn try_from(node: YamlNode) -> Result<Self, Self::Error> {
        let mut builder = Node::builder(node.identity)
            .with_endpoints(node.endpoints)
            .with_display_name(node.display_name)
            .with_keys(node.keys);

        for (k, v) in node.metadata {
            builder = builder.with_metadata(k, v);
        }

        builder.build()
    }
}
