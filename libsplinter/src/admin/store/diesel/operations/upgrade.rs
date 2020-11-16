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

//! Provides the "upgrade proposal to circuit" operation for the `DieselAdminServiceStore`.

use diesel::prelude::*;

use crate::admin::store::{error::AdminServiceStoreError, CircuitBuilder, CircuitNode, Service};
use crate::error::InvalidStateError;

use super::{
    add_circuit::AdminServiceStoreAddCircuitOperation,
    get_proposal::AdminServiceStoreFetchProposalOperation,
    remove_proposal::AdminServiceStoreRemoveProposalOperation, AdminServiceStoreOperations,
};

pub(in crate::admin::store::diesel) trait AdminServiceStoreUpgradeProposalToCircuitOperation {
    fn upgrade_proposal_to_circuit(&self, circuit_id: &str) -> Result<(), AdminServiceStoreError>;
}

#[cfg(all(feature = "admin-service-store-postgres", feature = "postgres"))]
impl<'a> AdminServiceStoreUpgradeProposalToCircuitOperation
    for AdminServiceStoreOperations<'a, diesel::pg::PgConnection>
{
    fn upgrade_proposal_to_circuit(&self, circuit_id: &str) -> Result<(), AdminServiceStoreError> {
        self.conn.transaction::<(), _, _>(|| {
            // Attempting to fetch the proposal to be upgraded. If not found, an error is returned.
            let proposal = match self.get_proposal(circuit_id)? {
                Some(proposal) => Ok(proposal),
                None => Err(AdminServiceStoreError::InvalidStateError(
                    InvalidStateError::with_message(String::from(
                        "CircuitProposal does not exist in AdminServiceStore",
                    )),
                )),
            }?;
            // Need to construct the `Circuit` from the `ProposedCircuit`
            let proposed_circuit = proposal.circuit();
            let circuit = CircuitBuilder::new()
                .with_circuit_id(proposed_circuit.circuit_id())
                .with_roster(
                    &proposed_circuit
                        .roster()
                        .iter()
                        .map(Service::from)
                        .collect::<Vec<Service>>(),
                )
                .with_members(
                    &proposed_circuit
                        .members()
                        .iter()
                        .map(|node| node.node_id().to_string())
                        .collect::<Vec<String>>(),
                )
                .with_authorization_type(proposed_circuit.authorization_type())
                .with_persistence(proposed_circuit.persistence())
                .with_durability(proposed_circuit.durability())
                .with_routes(proposed_circuit.routes())
                .with_circuit_management_type(proposed_circuit.circuit_management_type())
                .build()
                .map_err(AdminServiceStoreError::InvalidStateError)?;

            let circuit_nodes = proposed_circuit
                .members()
                .iter()
                .map(CircuitNode::from)
                .collect::<Vec<CircuitNode>>();

            self.remove_proposal(proposal.circuit_id())
                .and_then(|_| self.add_circuit(circuit, circuit_nodes))?;
            Ok(())
        })
    }
}

#[cfg(feature = "sqlite")]
impl<'a> AdminServiceStoreUpgradeProposalToCircuitOperation
    for AdminServiceStoreOperations<'a, diesel::sqlite::SqliteConnection>
{
    fn upgrade_proposal_to_circuit(&self, circuit_id: &str) -> Result<(), AdminServiceStoreError> {
        self.conn.transaction::<(), _, _>(|| {
            // Attempting to fetch the proposal to be upgraded. If not found, an error is returned.
            let proposal = match self.get_proposal(circuit_id)? {
                Some(proposal) => Ok(proposal),
                None => Err(AdminServiceStoreError::InvalidStateError(
                    InvalidStateError::with_message(String::from(
                        "CircuitProposal does not exist in AdminServiceStore",
                    )),
                )),
            }?;
            // Need to construct the `Circuit` from the `ProposedCircuit`
            let proposed_circuit = proposal.circuit();
            let circuit = CircuitBuilder::new()
                .with_circuit_id(proposed_circuit.circuit_id())
                .with_roster(
                    &proposed_circuit
                        .roster()
                        .iter()
                        .map(Service::from)
                        .collect::<Vec<Service>>(),
                )
                .with_members(
                    &proposed_circuit
                        .members()
                        .iter()
                        .map(|node| node.node_id().to_string())
                        .collect::<Vec<String>>(),
                )
                .with_authorization_type(proposed_circuit.authorization_type())
                .with_persistence(proposed_circuit.persistence())
                .with_durability(proposed_circuit.durability())
                .with_routes(proposed_circuit.routes())
                .with_circuit_management_type(proposed_circuit.circuit_management_type())
                .build()
                .map_err(AdminServiceStoreError::InvalidStateError)?;

            let circuit_nodes = proposed_circuit
                .members()
                .iter()
                .map(CircuitNode::from)
                .collect::<Vec<CircuitNode>>();

            self.remove_proposal(proposal.circuit_id())
                .and_then(|_| self.add_circuit(circuit, circuit_nodes))?;
            Ok(())
        })
    }
}
