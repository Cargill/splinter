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

//! Provides database models for the `DieselRegistry`.

use crate::registry::Node;

use super::schema::{
    splinter_nodes, splinter_nodes_endpoints, splinter_nodes_keys, splinter_nodes_metadata,
};

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "splinter_nodes"]
#[primary_key(identity)]
pub struct NodesModel {
    pub identity: String,
    pub display_name: String,
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "splinter_nodes_endpoints"]
#[belongs_to(NodesModel, foreign_key = "identity")]
#[primary_key(identity, endpoint)]
pub struct NodeEndpointsModel {
    pub identity: String,
    pub endpoint: String,
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable)]
#[table_name = "splinter_nodes_keys"]
#[belongs_to(NodesModel, foreign_key = "identity")]
#[primary_key(identity, key)]
pub struct NodeKeysModel {
    pub identity: String,
    pub key: String,
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable)]
#[table_name = "splinter_nodes_metadata"]
#[belongs_to(NodesModel, foreign_key = "identity")]
#[primary_key(identity, key)]
pub struct NodeMetadataModel {
    pub identity: String,
    pub key: String,
    pub value: String,
}

impl From<&Node> for NodesModel {
    fn from(node: &Node) -> Self {
        Self {
            identity: node.identity.clone(),
            display_name: node.display_name.clone(),
        }
    }
}

impl From<&Node> for Vec<NodeEndpointsModel> {
    fn from(node: &Node) -> Self {
        node.endpoints
            .iter()
            .map(|endpoint| NodeEndpointsModel {
                identity: node.identity.clone(),
                endpoint: endpoint.clone(),
            })
            .collect()
    }
}

impl From<&Node> for Vec<NodeKeysModel> {
    fn from(node: &Node) -> Self {
        node.keys
            .iter()
            .map(|key| NodeKeysModel {
                identity: node.identity.clone(),
                key: key.clone(),
            })
            .collect()
    }
}

impl From<&Node> for Vec<NodeMetadataModel> {
    fn from(node: &Node) -> Self {
        node.metadata
            .iter()
            .map(|(key, value)| NodeMetadataModel {
                identity: node.identity.clone(),
                key: key.clone(),
                value: value.clone(),
            })
            .collect()
    }
}
