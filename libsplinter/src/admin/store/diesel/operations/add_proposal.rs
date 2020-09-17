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

//! Provides the "add proposal" operation for the `DieselAdminServiceStore`.

use diesel::{dsl::insert_into, prelude::*};

use super::AdminServiceStoreOperations;
use crate::admin::store::{
    diesel::{
        models::{
            CircuitProposalModel, ProposedCircuitModel, ProposedNodeEndpointModel,
            ProposedNodeModel, ProposedServiceAllowedNodeModel, ProposedServiceArgumentModel,
            ProposedServiceModel, VoteRecordModel,
        },
        schema::{
            circuit_proposal, proposed_circuit, proposed_node, proposed_node_endpoint,
            proposed_service, proposed_service_allowed_node, proposed_service_argument,
            vote_record,
        },
    },
    error::AdminServiceStoreError,
    CircuitProposal,
};

pub(in crate::admin::store::diesel) trait AdminServiceStoreAddProposalOperation {
    fn add_proposal(&self, proposal: CircuitProposal) -> Result<(), AdminServiceStoreError>;
}

#[cfg(feature = "postgres")]
impl<'a> AdminServiceStoreAddProposalOperation
    for AdminServiceStoreOperations<'a, diesel::pg::PgConnection>
{
    fn add_proposal(&self, proposal: CircuitProposal) -> Result<(), AdminServiceStoreError> {
        // Insert `CircuitProposal` and all associated types into database after verifying that
        // the proposal exists
        self.conn.transaction::<(), _, _>(|| {
            // Check if a `CircuitProposal` already exists with the given `circuit_id`
            if circuit_proposal::table
                .filter(circuit_proposal::circuit_id.eq(&proposal.circuit_id))
                .first::<CircuitProposalModel>(self.conn)
                .optional()
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Diesel error occurred fetching Proposal"),
                    source: Box::new(err),
                })?
                .is_some()
            {
                return Err(AdminServiceStoreError::OperationError {
                    context: String::from("CircuitProposal already exists in AdminServiceStore"),
                    source: None,
                });
            }

            // Insert the database model of the `CircuitProposal`
            let circuit_proposal_model = CircuitProposalModel::from(&proposal);
            insert_into(circuit_proposal::table)
                .values(circuit_proposal_model)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert CircuitProposal"),
                    source: Box::new(err),
                })?;
            // Insert `ProposedCircuitModel`, representing the `proposed_circuit` of a `CircuitProposal`
            let proposed_circuit_model = ProposedCircuitModel::from(&proposal.circuit);
            insert_into(proposed_circuit::table)
                .values(proposed_circuit_model)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert ProposedCircuit"),
                    source: Box::new(err),
                })?;
            // Insert `members` of a `ProposedCircuit`
            let proposed_members: Vec<ProposedNodeModel> = Vec::from(&proposal.circuit);
            insert_into(proposed_node::table)
                .values(proposed_members)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert ProposedNode"),
                    source: Box::new(err),
                })?;
            // Insert the node `endpoints` and the proposed `members` of a `ProposedCircuit`
            let proposed_member_endpoints: Vec<ProposedNodeEndpointModel> =
                Vec::from(&proposal.circuit);
            insert_into(proposed_node_endpoint::table)
                .values(proposed_member_endpoints)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert ProposedNode's endpoint"),
                    source: Box::new(err),
                })?;
            // Insert `roster`, list of `Services` of a `ProposedCircuit`
            let proposed_services: Vec<ProposedServiceModel> = Vec::from(&proposal.circuit);
            insert_into(proposed_service::table)
                .values(proposed_services)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert ProposedService"),
                    source: Box::new(err),
                })?;
            // Insert `service_arguments` from the `Services` inserted above
            let proposed_service_argument: Vec<ProposedServiceArgumentModel> =
                Vec::from(&proposal.circuit);
            insert_into(proposed_service_argument::table)
                .values(proposed_service_argument)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert ProposedService's arguments"),
                    source: Box::new(err),
                })?;
            // Insert `allowed_nodes` from the `Services` inserted above
            let proposed_service_allowed_node: Vec<ProposedServiceAllowedNodeModel> =
                Vec::from(&proposal.circuit);
            insert_into(proposed_service_allowed_node::table)
                .values(proposed_service_allowed_node)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert ProposedService's allowed_nodes"),
                    source: Box::new(err),
                })?;
            // Insert `votes` from the `CircuitProposal`
            let vote_records: Vec<VoteRecordModel> = Vec::from(&proposal);
            insert_into(vote_record::table)
                .values(vote_records)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert CircuitProposal's vote_records"),
                    source: Box::new(err),
                })?;

            Ok(())
        })
    }
}

#[cfg(feature = "sqlite")]
impl<'a> AdminServiceStoreAddProposalOperation
    for AdminServiceStoreOperations<'a, diesel::sqlite::SqliteConnection>
{
    fn add_proposal(&self, proposal: CircuitProposal) -> Result<(), AdminServiceStoreError> {
        // Insert `CircuitProposal` and all associated types into database
        self.conn.transaction::<(), _, _>(|| {
            // Check if a `CircuitProposal` already exists with the given `circuit_id`, in which
            // case an error is returned.
            if circuit_proposal::table
                .filter(circuit_proposal::circuit_id.eq(&proposal.circuit_id))
                .first::<CircuitProposalModel>(self.conn)
                .optional()
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Diesel error occurred fetching Proposal"),
                    source: Box::new(err),
                })?
                .is_some()
            {
                return Err(AdminServiceStoreError::OperationError {
                    context: String::from("CircuitProposal already exists in AdminServiceStore"),
                    source: None,
                });
            }

            // Insert the database model of the `CircuitProposal`
            let circuit_proposal_model = CircuitProposalModel::from(&proposal);
            insert_into(circuit_proposal::table)
                .values(circuit_proposal_model)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert CircuitProposal"),
                    source: Box::new(err),
                })?;
            // Insert `ProposedCircuitModel`, representing the `proposed_circuit` of a `CircuitProposal`
            let proposed_circuit_model = ProposedCircuitModel::from(&proposal.circuit);
            insert_into(proposed_circuit::table)
                .values(proposed_circuit_model)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert ProposedCircuit"),
                    source: Box::new(err),
                })?;
            // Insert `members` of a `ProposedCircuit`
            let proposed_members: Vec<ProposedNodeModel> = Vec::from(&proposal.circuit);
            insert_into(proposed_node::table)
                .values(proposed_members)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert ProposedNode"),
                    source: Box::new(err),
                })?;
            // Insert the node `endpoints` the proposed `members` of a `ProposedCircuit`
            let proposed_member_endpoints: Vec<ProposedNodeEndpointModel> =
                Vec::from(&proposal.circuit);
            insert_into(proposed_node_endpoint::table)
                .values(proposed_member_endpoints)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert ProposedNode's endpoint"),
                    source: Box::new(err),
                })?;
            // Insert `roster`, list of `Services` of a `ProposedCircuit`
            let proposed_services: Vec<ProposedServiceModel> = Vec::from(&proposal.circuit);
            insert_into(proposed_service::table)
                .values(proposed_services)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert ProposedService"),
                    source: Box::new(err),
                })?;
            // Insert `service_arguments` from the `Services` inserted above
            let proposed_service_argument: Vec<ProposedServiceArgumentModel> =
                Vec::from(&proposal.circuit);
            insert_into(proposed_service_argument::table)
                .values(proposed_service_argument)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert ProposedService's arguments"),
                    source: Box::new(err),
                })?;
            // Insert `allowed_nodes` from the `Services` inserted above
            let proposed_service_allowed_node: Vec<ProposedServiceAllowedNodeModel> =
                Vec::from(&proposal.circuit);
            insert_into(proposed_service_allowed_node::table)
                .values(proposed_service_allowed_node)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert ProposedService's allowed_nodes"),
                    source: Box::new(err),
                })?;
            // Insert `votes` from the `CircuitProposal`
            let vote_records: Vec<VoteRecordModel> = Vec::from(&proposal);
            insert_into(vote_record::table)
                .values(vote_records)
                .execute(self.conn)
                .map_err(|err| AdminServiceStoreError::QueryError {
                    context: String::from("Unable to insert CircuitProposal's vote_records"),
                    source: Box::new(err),
                })?;

            Ok(())
        })
    }
}
