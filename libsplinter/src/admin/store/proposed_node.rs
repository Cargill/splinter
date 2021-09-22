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

//! Structs for building proposed nodes

use crate::admin::messages;
use crate::error::InvalidStateError;
use crate::protos::admin;

/// Native representation of a node in a proposed circuit
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposedNode {
    node_id: String,
    endpoints: Vec<String>,
    public_key: Option<Vec<u8>>,
}

impl ProposedNode {
    /// Returns the ID of the proposed node
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Returns the list of endpoints that belong to the proposed node
    pub fn endpoints(&self) -> &[String] {
        &self.endpoints
    }

    /// Returns the public key that belongs to the proposed node
    pub fn public_key(&self) -> &Option<Vec<u8>> {
        &self.public_key
    }

    pub fn into_proto(self) -> admin::SplinterNode {
        let mut proto = admin::SplinterNode::new();

        proto.set_node_id(self.node_id);
        proto.set_endpoints(self.endpoints.into());

        if let Some(public_key) = self.public_key {
            proto.set_public_key(public_key);
        }

        proto
    }

    pub fn from_proto(mut proto: admin::SplinterNode) -> Self {
        let public_key = {
            let public_key = proto.take_public_key();
            if public_key.is_empty() {
                None
            } else {
                Some(public_key)
            }
        };

        Self {
            node_id: proto.take_node_id(),
            endpoints: proto.take_endpoints().into(),
            public_key,
        }
    }
}

/// Builder for creating a `ProposedNode`
#[derive(Default, Clone)]
pub struct ProposedNodeBuilder {
    node_id: Option<String>,
    endpoints: Option<Vec<String>>,
    public_key: Option<Vec<u8>>,
}

impl ProposedNodeBuilder {
    /// Creates a `ProposedNodeBuider`
    pub fn new() -> Self {
        ProposedNodeBuilder::default()
    }

    /// Returns the unique node ID
    pub fn node_id(&self) -> Option<String> {
        self.node_id.clone()
    }

    /// Returns the list of endpoints for the node
    pub fn endpoints(&self) -> Option<Vec<String>> {
        self.endpoints.clone()
    }

    /// Returns the publice key for the node
    pub fn public_key(&self) -> Option<Vec<u8>> {
        self.public_key.clone()
    }

    /// Sets the node ID
    ///
    /// # Arguments
    ///
    ///  * `node_id` - The unique node ID for node
    pub fn with_node_id(mut self, node_id: &str) -> ProposedNodeBuilder {
        self.node_id = Some(node_id.into());
        self
    }

    /// Sets the endpoints
    ///
    /// # Arguments
    ///
    ///  * `endpoints` - The list of endpoints for the node
    pub fn with_endpoints(mut self, endpoints: &[String]) -> ProposedNodeBuilder {
        self.endpoints = Some(endpoints.into());
        self
    }

    /// Sets the public key
    ///
    /// # Arguments
    ///
    ///  * `public_key` - The bytes of the node's public key
    pub fn with_public_key(mut self, public_key: &[u8]) -> ProposedNodeBuilder {
        self.public_key = Some(public_key.into());
        self
    }

    /// Builds the `ProposedNode`
    ///
    /// Returns an error if the node ID or endpoints are not set
    pub fn build(self) -> Result<ProposedNode, InvalidStateError> {
        let node_id = self.node_id.ok_or_else(|| {
            InvalidStateError::with_message("unable to build, missing field: `node_id`".to_string())
        })?;

        let endpoints = self.endpoints.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `endpoints`".to_string(),
            )
        })?;

        let node = ProposedNode {
            node_id,
            endpoints,
            public_key: self.public_key,
        };

        Ok(node)
    }
}

impl From<&messages::SplinterNode> for ProposedNode {
    fn from(admin_node: &messages::SplinterNode) -> ProposedNode {
        ProposedNode {
            node_id: admin_node.node_id.to_string(),
            endpoints: admin_node.endpoints.to_vec(),
            public_key: admin_node.public_key.clone(),
        }
    }
}
