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

//! Provides the "fetch circuit" operation for the `DieselAdminServiceStore`.

use diesel::prelude::*;
use diesel::sql_types::{Binary, Integer, Nullable, Text};
use std::collections::HashMap;
use std::convert::TryFrom;

use super::{list_services::AdminServiceStoreListServicesOperation, AdminServiceStoreOperations};
use crate::admin::store::{
    diesel::{
        models::{CircuitMemberModel, CircuitModel, NodeEndpointModel},
        schema::{circuit, circuit_member, node_endpoint},
    },
    error::AdminServiceStoreError,
    AuthorizationType, Circuit, CircuitBuilder, CircuitNode, CircuitNodeBuilder, CircuitStatus,
    DurabilityType, PersistenceType, RouteType, Service,
};
use crate::error::InvalidStateError;

pub(in crate::admin::store::diesel) trait AdminServiceStoreFetchCircuitOperation {
    fn get_circuit(&self, circuit_id: &str) -> Result<Option<Circuit>, AdminServiceStoreError>;
}

impl<'a, C> AdminServiceStoreFetchCircuitOperation for AdminServiceStoreOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<Text, C::Backend>,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    i32: diesel::deserialize::FromSql<Integer, C::Backend>,
    i16: diesel::deserialize::FromSql<diesel::sql_types::SmallInt, C::Backend>,
    CircuitMemberModel: diesel::Queryable<(Text, Text, Integer, Nullable<Binary>), C::Backend>,
{
    fn get_circuit(&self, circuit_id: &str) -> Result<Option<Circuit>, AdminServiceStoreError> {
        self.conn.transaction::<Option<Circuit>, _, _>(|| {
            // Retrieve the `circuit` entry with the matching `circuit_id`
            // return None if the `circuit` does not exist
            let circuit: CircuitModel = match circuit::table
                .select(circuit::all_columns)
                .filter(circuit::circuit_id.eq(circuit_id.to_string()))
                .first::<CircuitModel>(self.conn)
                .optional()?
            {
                Some(circuit) => circuit,
                None => return Ok(None),
            };

            // Collecting the members of the `Circuit`
            let nodes_info: Vec<(CircuitMemberModel, NodeEndpointModel)> = circuit_member::table
                .filter(circuit_member::circuit_id.eq(circuit_id.to_string()))
                // As `circuit_member` and `node_endpoint` have a one-to-many relationship, this join
                // will return all matching entries as there are `node_endpoint` entries.
                .order(circuit_member::position)
                .inner_join(
                    node_endpoint::table.on(circuit_member::node_id.eq(node_endpoint::node_id)),
                )
                .load(self.conn)?;

            let mut node_map: HashMap<String, Vec<String>> = HashMap::new();
            let mut nodes: HashMap<String, CircuitMemberModel> = HashMap::new();
            // Iterate over the list of node data retrieved from the database, in order to collect all
            // endpoints associated with the `node_ids` in a HashMap.
            nodes_info.into_iter().for_each(|(node, node_endpoint)| {
                if let Some(endpoint_list) = node_map.get_mut(&node.node_id) {
                    endpoint_list.push(node_endpoint.endpoint);
                    // Ensure only unique endpoints are added to the node's endpoint list
                    endpoint_list.sort();
                    endpoint_list.dedup();
                } else {
                    node_map.insert(node.node_id.to_string(), vec![node_endpoint.endpoint]);
                }

                if !nodes.contains_key(&node.node_id) {
                    nodes.insert(node.node_id.to_string(), node);
                }
            });

            let mut nodes_vec: Vec<CircuitMemberModel> =
                nodes.into_iter().map(|(_, node)| node).collect();
            nodes_vec.sort_by_key(|node| node.position);

            // Collecting services associated with the `Circuit` using the `list_services` method,
            // which provides a list of the `Services` with the matching `circuit_id`.
            let services: Vec<Service> = self.list_services(circuit_id)?.collect();
            let circuit_members: Vec<CircuitNode> = nodes_vec
                .iter()
                .map(|member| {
                    let mut builder = CircuitNodeBuilder::new().with_node_id(&member.node_id);

                    if let Some(endpoints) = node_map.get(&member.node_id) {
                        builder = builder.with_endpoints(endpoints);
                    }

                    #[cfg(feature = "challenge-authorization")]
                    {
                        if let Some(public_key) = &member.public_key {
                            builder = builder.with_public_key(public_key);
                        }
                    }

                    builder.build()
                })
                .collect::<Result<Vec<CircuitNode>, InvalidStateError>>()
                .map_err(AdminServiceStoreError::InvalidStateError)?;

            let mut builder = CircuitBuilder::new()
                .with_circuit_id(&circuit.circuit_id)
                .with_roster(&services)
                .with_members(&circuit_members)
                .with_authorization_type(&AuthorizationType::try_from(circuit.authorization_type)?)
                .with_persistence(&PersistenceType::try_from(circuit.persistence)?)
                .with_durability(&DurabilityType::try_from(circuit.durability)?)
                .with_routes(&RouteType::try_from(circuit.routes)?)
                .with_circuit_management_type(&circuit.circuit_management_type)
                .with_circuit_version(circuit.circuit_version)
                .with_circuit_status(&CircuitStatus::from(&circuit.circuit_status));

            // if display name is set, add to builder
            if let Some(display_name) = circuit.display_name {
                builder = builder.with_display_name(&display_name);
            }

            Ok(Some(
                builder
                    .build()
                    .map_err(AdminServiceStoreError::InvalidStateError)?,
            ))
        })
    }
}
