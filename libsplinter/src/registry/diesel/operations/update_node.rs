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

//! Provides the "update node" operation for the `DieselRegistry`.

use diesel::{
    dsl::{delete, insert_into, not, update},
    prelude::*,
};

use crate::error::InvalidStateError;
use crate::registry::{
    diesel::{
        models::{NodeEndpointsModel, NodeKeysModel, NodeMetadataModel, NodesModel},
        schema::{
            splinter_nodes, splinter_nodes_endpoints, splinter_nodes_keys, splinter_nodes_metadata,
        },
    },
    Node, RegistryError,
};

use super::RegistryOperations;

pub(in crate::registry::diesel) trait RegistryUpdateNodeOperation {
    fn update_node(&self, node: Node) -> Result<(), RegistryError>;
}

#[cfg(feature = "postgres")]
impl<'a> RegistryUpdateNodeOperation for RegistryOperations<'a, diesel::pg::PgConnection> {
    fn update_node(&self, node: Node) -> Result<(), RegistryError> {
        self.conn.transaction::<(), _, _>(|| {
            // Verify that the node's endpoints are unique.
            let filters = node
                .endpoints
                .iter()
                .map(|endpoint| endpoint.to_string())
                .collect::<Vec<_>>();

            let duplicate_endpoint = splinter_nodes_endpoints::table
                .filter(splinter_nodes_endpoints::endpoint.eq_any(filters))
                .filter(not(splinter_nodes_endpoints::identity.eq(&node.identity)))
                .first::<NodeEndpointsModel>(self.conn)
                .optional()?;

            if let Some(endpoint) = duplicate_endpoint {
                return Err(RegistryError::InvalidStateError(
                    InvalidStateError::with_message(format!(
                        "another node with endpoint {} exists",
                        endpoint.endpoint
                    )),
                ));
            }

            // Check if the node exists
            let existing_node = splinter_nodes::table
                .find(&node.identity)
                .first::<NodesModel>(self.conn)
                .optional()?;

            if existing_node.is_some() {
                // Update existing node
                update(splinter_nodes::table.find(&node.identity))
                    .set(splinter_nodes::display_name.eq(&node.display_name))
                    .execute(self.conn)?;

                // Remove old endpoints, keys, and metadata for the node
                delete(
                    splinter_nodes_endpoints::table
                        .filter(splinter_nodes_endpoints::identity.eq(&node.identity)),
                )
                .execute(self.conn)?;

                delete(
                    splinter_nodes_keys::table
                        .filter(splinter_nodes_keys::identity.eq(&node.identity)),
                )
                .execute(self.conn)?;

                delete(
                    splinter_nodes_metadata::table
                        .filter(splinter_nodes_metadata::identity.eq(&node.identity)),
                )
                .execute(self.conn)?;

                // Add endpoints, keys, and metadata for the node
                let endpoints: Vec<NodeEndpointsModel> = Vec::from(&node);
                insert_into(splinter_nodes_endpoints::table)
                    .values(&endpoints)
                    .execute(self.conn)?;

                let keys: Vec<NodeKeysModel> = Vec::from(&node);
                insert_into(splinter_nodes_keys::table)
                    .values(&keys)
                    .execute(self.conn)?;

                let metadata: Vec<NodeMetadataModel> = Vec::from(&node);
                insert_into(splinter_nodes_metadata::table)
                    .values(&metadata)
                    .execute(self.conn)?;

                Ok(())
            } else {
                Err(RegistryError::InvalidStateError(
                    InvalidStateError::with_message(format!(
                        "Node does not exist: {}",
                        &node.identity
                    )),
                ))
            }
        })
    }
}

#[cfg(feature = "sqlite")]
impl<'a> RegistryUpdateNodeOperation for RegistryOperations<'a, diesel::sqlite::SqliteConnection> {
    fn update_node(&self, node: Node) -> Result<(), RegistryError> {
        self.conn.transaction::<(), _, _>(|| {
            // Verify that the node's endpoints are unique.
            let filters = node
                .endpoints
                .iter()
                .map(|endpoint| endpoint.to_string())
                .collect::<Vec<_>>();

            let duplicate_endpoint = splinter_nodes_endpoints::table
                .filter(splinter_nodes_endpoints::endpoint.eq_any(filters))
                .filter(not(splinter_nodes_endpoints::identity.eq(&node.identity)))
                .first::<NodeEndpointsModel>(self.conn)
                .optional()?;

            if let Some(endpoint) = duplicate_endpoint {
                return Err(RegistryError::InvalidStateError(
                    InvalidStateError::with_message(format!(
                        "another node with endpoint {} exists",
                        endpoint.endpoint
                    )),
                ));
            }

            // Check if the node exists
            let existing_node = splinter_nodes::table
                .find(&node.identity)
                .first::<NodesModel>(self.conn)
                .optional()?;

            if existing_node.is_some() {
                // Update existing node
                update(splinter_nodes::table.find(&node.identity))
                    .set(splinter_nodes::display_name.eq(&node.display_name))
                    .execute(self.conn)?;

                // Remove old endpoints, keys, and metadata for the node
                delete(
                    splinter_nodes_endpoints::table
                        .filter(splinter_nodes_endpoints::identity.eq(&node.identity)),
                )
                .execute(self.conn)?;

                delete(
                    splinter_nodes_keys::table
                        .filter(splinter_nodes_keys::identity.eq(&node.identity)),
                )
                .execute(self.conn)?;

                delete(
                    splinter_nodes_metadata::table
                        .filter(splinter_nodes_metadata::identity.eq(&node.identity)),
                )
                .execute(self.conn)?;

                // Add endpoints, keys, and metadata for the node
                let endpoints: Vec<NodeEndpointsModel> = Vec::from(&node);
                insert_into(splinter_nodes_endpoints::table)
                    .values(&endpoints)
                    .execute(self.conn)?;

                let keys: Vec<NodeKeysModel> = Vec::from(&node);
                insert_into(splinter_nodes_keys::table)
                    .values(&keys)
                    .execute(self.conn)?;

                let metadata: Vec<NodeMetadataModel> = Vec::from(&node);
                insert_into(splinter_nodes_metadata::table)
                    .values(&metadata)
                    .execute(self.conn)?;

                Ok(())
            } else {
                Err(RegistryError::InvalidStateError(
                    InvalidStateError::with_message(format!(
                        "Node does not exist: {}",
                        &node.identity
                    )),
                ))
            }
        })
    }
}
