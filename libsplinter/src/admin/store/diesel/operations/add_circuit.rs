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

//! Provides the "add circuit" operation for the `DieselAdminServiceStore`.

use std::collections::HashMap;

use diesel::{dsl::insert_into, prelude::*};

use super::AdminServiceStoreOperations;
use crate::admin::store::{
    diesel::{
        models::{
            CircuitMemberModel, CircuitModel, NodeEndpointModel, ServiceArgumentModel, ServiceModel,
        },
        schema::{circuit, circuit_member, node_endpoint, service, service_argument},
    },
    error::AdminServiceStoreError,
    Circuit, CircuitNode,
};

pub(in crate::admin::store::diesel) trait AdminServiceStoreAddCircuitOperation {
    fn add_circuit(
        &self,
        circuit: Circuit,
        nodes: Vec<CircuitNode>,
    ) -> Result<(), AdminServiceStoreError>;
}

#[cfg(feature = "postgres")]
impl<'a> AdminServiceStoreAddCircuitOperation
    for AdminServiceStoreOperations<'a, diesel::pg::PgConnection>
{
    fn add_circuit(
        &self,
        circuit: Circuit,
        nodes: Vec<CircuitNode>,
    ) -> Result<(), AdminServiceStoreError> {
        self.conn.transaction::<(), _, _>(|| {
            // Check if the circuit already exists in the `AdminServiceStore`, in which case
            // an error is returned.
            if circuit::table
                .filter(circuit::circuit_id.eq(circuit.circuit_id()))
                .first::<CircuitModel>(self.conn)
                .optional()
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Error occurred fetching Circuit"),
                    source: Box::new(err),
                })?
                .is_some()
            {
                return Err(AdminServiceStoreError::OperationError {
                    context: String::from("Circuit already exists in AdminServiceStore"),
                    source: None,
                });
            }

            // Create a `CircuitModel` from the `Circuit` to add to database
            let circuit_model: CircuitModel = CircuitModel::from(&circuit);
            insert_into(circuit::table)
                .values(circuit_model)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert Circuit"),
                    source: Box::new(err),
                })?;
            // Create a list of circuit members from `nodes`
            let circuit_members: Vec<CircuitMemberModel> = nodes
                .iter()
                .map(|node| CircuitMemberModel {
                    circuit_id: circuit.circuit_id().into(),
                    node_id: node.node_id().into(),
                })
                .collect();
            insert_into(circuit_member::table)
                .values(circuit_members)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert Circuit members"),
                    source: Box::new(err),
                })?;
            // Iterate over the list of `CircuitNodes` to extract the `node_id` and `endpoints`, to
            // convert them into the `NodeEndpointModel`. Then, verify the `node_id` does not
            // already have associated `node_endpoint` entries before inserting the list of
            // `NodeEndpointModel`.
            for (node_id, endpoints) in nodes
                .iter()
                .map(|node| {
                    (
                        node.node_id().into(),
                        node.endpoints()
                            .iter()
                            .map(|endpoint| NodeEndpointModel {
                                node_id: node.node_id().into(),
                                endpoint: endpoint.into(),
                            })
                            .collect::<Vec<NodeEndpointModel>>(),
                    )
                })
                .collect::<HashMap<String, Vec<NodeEndpointModel>>>()
                .into_iter()
            {
                if let Some(0) = node_endpoint::table
                    .filter(node_endpoint::node_id.eq(&node_id))
                    .count()
                    .first(self.conn)
                    .optional()
                    .map_err(|err| AdminServiceStoreError::QueryError {
                        context: String::from("Error occurred counting CircuitNode endpoints"),
                        source: Box::new(err),
                    })?
                {
                    insert_into(node_endpoint::table)
                        .values(endpoints)
                        .execute(self.conn)
                        .map_err(|err| AdminServiceStoreError::QueryError {
                            context: String::from("Unable to insert Circuit members"),
                            source: Box::new(err),
                        })?;
                }
            }

            // Build `Services` and all associated data from `circuit`
            let circuit_services: Vec<ServiceModel> = Vec::from(&circuit);
            insert_into(service::table)
                .values(&circuit_services)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert Services"),
                    source: Box::new(err),
                })?;
            let service_argument: Vec<ServiceArgumentModel> = Vec::from(&circuit);
            insert_into(service_argument::table)
                .values(&service_argument)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert Service arguments"),
                    source: Box::new(err),
                })?;

            Ok(())
        })
    }
}

#[cfg(feature = "sqlite")]
impl<'a> AdminServiceStoreAddCircuitOperation
    for AdminServiceStoreOperations<'a, diesel::sqlite::SqliteConnection>
{
    fn add_circuit(
        &self,
        circuit: Circuit,
        nodes: Vec<CircuitNode>,
    ) -> Result<(), AdminServiceStoreError> {
        self.conn.transaction::<(), _, _>(|| {
            // Check if the circuit already exists in the `AdminServiceStore`, in which case
            // an error is returned.
            if circuit::table
                .filter(circuit::circuit_id.eq(circuit.circuit_id()))
                .first::<CircuitModel>(self.conn)
                .optional()
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Error occurred fetching Circuit"),
                    source: Box::new(err),
                })?
                .is_some()
            {
                return Err(AdminServiceStoreError::OperationError {
                    context: String::from("Circuit already exists in AdminServiceStore"),
                    source: None,
                });
            }

            // Create a `CircuitModel` from the `Circuit` to add to database
            let circuit_model: CircuitModel = CircuitModel::from(&circuit);
            insert_into(circuit::table)
                .values(circuit_model)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert Circuit"),
                    source: Box::new(err),
                })?;
            // Create a list of circuit members from `nodes`
            let circuit_member: Vec<CircuitMemberModel> = nodes
                .iter()
                .map(|node| CircuitMemberModel {
                    circuit_id: circuit.circuit_id().into(),
                    node_id: node.node_id().into(),
                })
                .collect();
            insert_into(circuit_member::table)
                .values(circuit_member)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert Circuit members"),
                    source: Box::new(err),
                })?;
            // Iterate over the list of `CircuitNodes` to extract the `node_id` and `endpoints`, to
            // convert them into the `NodeEndpointModel`. Then, verify the `node_id` does not
            // already have associated `node_endpoint` entries before inserting the list of
            // `NodeEndpointModel`.
            for (node_id, endpoints) in nodes
                .iter()
                .map(|node| {
                    (
                        node.node_id().into(),
                        node.endpoints()
                            .iter()
                            .map(|endpoint| NodeEndpointModel {
                                node_id: node.node_id().into(),
                                endpoint: endpoint.into(),
                            })
                            .collect::<Vec<NodeEndpointModel>>(),
                    )
                })
                .collect::<HashMap<String, Vec<NodeEndpointModel>>>()
                .into_iter()
            {
                if let Some(0) = node_endpoint::table
                    .filter(node_endpoint::node_id.eq(&node_id))
                    .count()
                    .first(self.conn)
                    .optional()
                    .map_err(|err| AdminServiceStoreError::QueryError {
                        context: String::from("Error occurred counting CircuitNode endpoints"),
                        source: Box::new(err),
                    })?
                {
                    insert_into(node_endpoint::table)
                        .values(endpoints)
                        .execute(self.conn)
                        .map_err(|err| AdminServiceStoreError::QueryError {
                            context: String::from("Unable to insert Circuit members"),
                            source: Box::new(err),
                        })?;
                }
            }

            // Build `Services` and all associated data from `circuit`
            let circuit_services: Vec<ServiceModel> = Vec::from(&circuit);
            insert_into(service::table)
                .values(&circuit_services)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert Services"),
                    source: Box::new(err),
                })?;
            let service_argument: Vec<ServiceArgumentModel> = Vec::from(&circuit);
            insert_into(service_argument::table)
                .values(&service_argument)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert Service arguments"),
                    source: Box::new(err),
                })?;

            Ok(())
        })
    }
}
