// Copyright 2018-2020 Cargill Incorporated
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

//! Structs for building circuits nodes
use crate::error::InvalidStateError;

use super::ProposedNode;

/// Native representation of a node included in circuit
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CircuitNode {
    id: String,
    endpoints: Vec<String>,
}

impl CircuitNode {
    /// Returns the ID of the node
    pub fn node_id(&self) -> &str {
        &self.id
    }

    /// Returns the list of endpoints that belong to the node
    pub fn endpoints(&self) -> &[String] {
        &self.endpoints
    }
}

impl From<&ProposedNode> for CircuitNode {
    fn from(proposed_node: &ProposedNode) -> Self {
        CircuitNode {
            id: proposed_node.node_id().into(),
            endpoints: proposed_node.endpoints().to_vec(),
        }
    }
}

impl From<ProposedNode> for CircuitNode {
    fn from(node: ProposedNode) -> Self {
        CircuitNode {
            id: node.node_id().into(),
            endpoints: node.endpoints().to_vec(),
        }
    }
}

/// Builder for creating a `CircutNode`
#[derive(Default, Clone)]
pub struct CircuitNodeBuilder {
    node_id: Option<String>,
    endpoints: Option<Vec<String>>,
}

impl CircuitNodeBuilder {
    /// Creates a `CircuitNodeBuilder`
    pub fn new() -> Self {
        CircuitNodeBuilder::default()
    }

    /// Returns the unique node ID
    pub fn node_id(&self) -> Option<String> {
        self.node_id.clone()
    }

    /// Returns the list of endpoints for the node
    pub fn endpoints(&self) -> Option<Vec<String>> {
        self.endpoints.clone()
    }

    /// Sets the node ID
    ///
    /// # Arguments
    ///
    ///  * `node_id` - The unique node ID for node
    pub fn with_node_id(mut self, node_id: &str) -> CircuitNodeBuilder {
        self.node_id = Some(node_id.into());
        self
    }

    /// Sets the endpoints
    ///
    /// # Arguments
    ///
    ///  * `endpoints` - The list of endpoints for the node
    pub fn with_endpoints(mut self, endpoints: &[String]) -> CircuitNodeBuilder {
        self.endpoints = Some(endpoints.into());
        self
    }

    /// Builds the `CircuitNode`
    ///
    /// Returns an error if the node ID or endpoints are not set
    pub fn build(self) -> Result<CircuitNode, InvalidStateError> {
        let node_id = self.node_id.ok_or_else(|| {
            InvalidStateError::with_message("unable to build, missing field: `node_id`".to_string())
        })?;

        let endpoints = self.endpoints.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `endpoints`".to_string(),
            )
        })?;

        let node = CircuitNode {
            id: node_id,
            endpoints,
        };

        Ok(node)
    }
}
