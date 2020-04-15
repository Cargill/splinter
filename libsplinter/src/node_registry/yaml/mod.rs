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

mod local;
#[cfg(feature = "registry-remote")]
mod remote;

use super::{InvalidNodeError, Node};

pub use local::LocalYamlNodeRegistry;
#[cfg(feature = "registry-remote")]
pub use remote::{RemoteYamlNodeRegistry, ShutdownHandle as RemoteYamlShutdownHandle};

fn validate_nodes(nodes: &[Node]) -> Result<(), InvalidNodeError> {
    for (idx, node) in nodes.iter().enumerate() {
        check_node_required_fields_are_not_empty(node)?;
        check_if_node_is_duplicate(node, &nodes[idx + 1..])?;
    }
    Ok(())
}

fn check_node_required_fields_are_not_empty(node: &Node) -> Result<(), InvalidNodeError> {
    if node.identity.is_empty() {
        Err(InvalidNodeError::EmptyIdentity)
    } else if node.endpoints.is_empty() {
        Err(InvalidNodeError::MissingEndpoints)
    } else if node.endpoints.iter().any(|endpoint| endpoint.is_empty()) {
        Err(InvalidNodeError::EmptyEndpoint)
    } else if node.display_name.is_empty() {
        Err(InvalidNodeError::EmptyDisplayName)
    } else {
        Ok(())
    }
}

fn check_if_node_is_duplicate(
    node: &Node,
    existing_nodes: &[Node],
) -> Result<(), InvalidNodeError> {
    existing_nodes.iter().try_for_each(|existing_node| {
        if existing_node.identity == node.identity {
            Err(InvalidNodeError::DuplicateIdentity(node.identity.clone()))
        } else if let Some(endpoint) = existing_node
            .endpoints
            .iter()
            .find(|endpoint| node.endpoints.contains(endpoint))
        {
            Err(InvalidNodeError::DuplicateEndpoint(endpoint.clone()))
        } else {
            Ok(())
        }
    })
}
