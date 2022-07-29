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

use std::collections::HashMap;
use std::convert::TryFrom;

use serde::{Deserialize, Serialize};
use splinter::registry::{InvalidNodeError, Node};
use splinter_rest_api_common::paging::v1::Paging;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ListNodesResponse<'a> {
    pub data: Vec<NodeResponse<'a>>,
    pub paging: Paging,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NodeResponse<'a> {
    pub identity: &'a str,
    pub endpoints: &'a [String],
    pub display_name: &'a str,
    pub keys: &'a [String],
    pub metadata: &'a HashMap<String, String>,
}

impl<'a> From<&'a Node> for NodeResponse<'a> {
    fn from(node: &'a Node) -> Self {
        Self {
            identity: node.identity(),
            endpoints: node.endpoints(),
            display_name: node.display_name(),
            keys: node.keys(),
            metadata: node.metadata(),
        }
    }
}

/// Used to deserialize add and update requests
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct NewNode {
    /// The Splinter identity of the node; must be non-empty and unique in the registry.
    pub identity: String,
    /// The endpoints the node can be reached at; at least one endpoint must be provided, and each
    /// endpoint must be non-empty and unique in the registry.
    pub endpoints: Vec<String>,
    /// A human-readable name for the node; must be non-empty.
    pub display_name: String,
    /// The list of public keys that are permitted to act on behalf of the node; at least one key
    /// must be provided, and each key must be non-empty.
    pub keys: Vec<String>,
    /// A map with node metadata.
    pub metadata: HashMap<String, String>,
}

impl TryFrom<NewNode> for Node {
    type Error = InvalidNodeError;

    fn try_from(node: NewNode) -> Result<Self, Self::Error> {
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
