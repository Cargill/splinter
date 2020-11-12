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

//! Provides the "update proposal" operation for the `DieselAdminServiceStore`.

use diesel::{
    dsl::{delete, insert_into, update},
    prelude::*,
};

use super::AdminServiceStoreOperations;
use crate::admin::store::{
    diesel::{
        models::{
            CircuitProposalModel, ProposedCircuitModel, ProposedNodeEndpointModel,
            ProposedNodeModel, ProposedServiceArgumentModel, ProposedServiceModel, VoteRecordModel,
        },
        schema::{
            circuit_proposal, proposed_circuit, proposed_node, proposed_node_endpoint,
            proposed_service, proposed_service_argument, vote_record,
        },
    },
    error::AdminServiceStoreError,
    CircuitProposal,
};
use crate::error::InvalidStateError;

pub(in crate::admin::store::diesel) trait AdminServiceStoreUpdateProposalOperation {
    fn update_proposal(&self, proposal: CircuitProposal) -> Result<(), AdminServiceStoreError>;
}

#[cfg(all(feature = "admin-service-store-postgres", feature = "postgres"))]
impl<'a> AdminServiceStoreUpdateProposalOperation
    for AdminServiceStoreOperations<'a, diesel::pg::PgConnection>
{
    fn update_proposal(&self, proposal: CircuitProposal) -> Result<(), AdminServiceStoreError> {
        self.conn.transaction::<(), _, _>(|| {
            // Verify the `circuit_proposal` entry to be updated exists
            circuit_proposal::table
                .filter(circuit_proposal::circuit_id.eq(proposal.circuit_id()))
                .first::<CircuitProposalModel>(self.conn)
                .optional()?
                .ok_or_else(|| {
                    AdminServiceStoreError::InvalidStateError(InvalidStateError::with_message(
                        String::from("CircuitProposal does not exist in AdminServiceStore"),
                    ))
                })?;

            // Update existing `CircuitProposal`
            let proposal_model = CircuitProposalModel::from(&proposal);
            update(circuit_proposal::table.find(proposal.circuit_id()))
                .set((
                    circuit_proposal::proposal_type.eq(proposal_model.proposal_type),
                    circuit_proposal::circuit_hash.eq(proposal_model.circuit_hash),
                    circuit_proposal::requester.eq(proposal_model.requester),
                    circuit_proposal::requester_node_id.eq(proposal_model.requester_node_id),
                ))
                .execute(self.conn)?;
            // Update existing `ProposedCircuit`
            let proposed_circuit_model = ProposedCircuitModel::from(proposal.circuit());
            update(proposed_circuit::table.find(proposal.circuit_id()))
                .set((
                    proposed_circuit::authorization_type
                        .eq(proposed_circuit_model.authorization_type),
                    proposed_circuit::persistence.eq(proposed_circuit_model.persistence),
                    proposed_circuit::durability.eq(proposed_circuit_model.durability),
                    proposed_circuit::routes.eq(proposed_circuit_model.routes),
                    proposed_circuit::circuit_management_type
                        .eq(proposed_circuit_model.circuit_management_type),
                    proposed_circuit::application_metadata
                        .eq(proposed_circuit_model.application_metadata),
                    proposed_circuit::comments.eq(proposed_circuit_model.comments),
                ))
                .execute(self.conn)?;

            // Delete existing data associated with the `CircuitProposal` and `ProposedCircuit`
            let node_ids: Vec<String> = proposed_node::table
                .filter(proposed_node::circuit_id.eq(proposal.circuit_id()))
                .select(proposed_node::node_id)
                .load(self.conn)?;

            delete(
                proposed_node::table.filter(proposed_node::circuit_id.eq(proposal.circuit_id())),
            )
            .execute(self.conn)?;
            delete(
                proposed_node_endpoint::table
                    .filter(proposed_node_endpoint::node_id.eq_any(&node_ids)),
            )
            .execute(self.conn)?;
            delete(
                proposed_service::table
                    .filter(proposed_service::circuit_id.eq(proposal.circuit_id())),
            )
            .execute(self.conn)?;
            delete(
                proposed_service_argument::table
                    .filter(proposed_service_argument::circuit_id.eq(proposal.circuit_id())),
            )
            .execute(self.conn)?;
            delete(vote_record::table.filter(vote_record::circuit_id.eq(proposal.circuit_id())))
                .execute(self.conn)?;
            // Insert the updated info for all of the `CircuitProposal` and `ProposedCircuit`
            // associated data
            // Insert `members` of a `ProposedCircuit`
            let proposed_members: Vec<ProposedNodeModel> = Vec::from(proposal.circuit());
            insert_into(proposed_node::table)
                .values(proposed_members)
                .execute(self.conn)?;
            // Insert the node `endpoints` the proposed `members` of a `ProposedCircuit`
            let proposed_member_endpoints: Vec<ProposedNodeEndpointModel> =
                Vec::from(proposal.circuit());
            insert_into(proposed_node_endpoint::table)
                .values(proposed_member_endpoints)
                .execute(self.conn)?;
            // Insert `roster`, list of `Services` of a `ProposedCircuit`
            let proposed_service: Vec<ProposedServiceModel> = Vec::from(proposal.circuit());
            insert_into(proposed_service::table)
                .values(proposed_service)
                .execute(self.conn)?;
            // Insert `service_arguments` from the `Services` inserted above
            let proposed_service_argument: Vec<ProposedServiceArgumentModel> =
                Vec::from(proposal.circuit());
            insert_into(proposed_service_argument::table)
                .values(proposed_service_argument)
                .execute(self.conn)?;
            // Insert `votes` from the `CircuitProposal`
            let vote_record: Vec<VoteRecordModel> = Vec::from(&proposal);
            insert_into(vote_record::table)
                .values(vote_record)
                .execute(self.conn)?;

            Ok(())
        })
    }
}

#[cfg(feature = "sqlite")]
impl<'a> AdminServiceStoreUpdateProposalOperation
    for AdminServiceStoreOperations<'a, diesel::sqlite::SqliteConnection>
{
    fn update_proposal(&self, proposal: CircuitProposal) -> Result<(), AdminServiceStoreError> {
        self.conn.transaction::<(), _, _>(|| {
            // Verify the `circuit_proposal` entry to be updated exists
            circuit_proposal::table
                .filter(circuit_proposal::circuit_id.eq(proposal.circuit_id()))
                .first::<CircuitProposalModel>(self.conn)
                .optional()?
                .ok_or_else(|| {
                    AdminServiceStoreError::InvalidStateError(InvalidStateError::with_message(
                        String::from("CircuitProposal does not exist in AdminServiceStore"),
                    ))
                })?;

            // Update existing `CircuitProposal`
            let proposal_model = CircuitProposalModel::from(&proposal);
            update(circuit_proposal::table.find(proposal.circuit_id()))
                .set((
                    circuit_proposal::proposal_type.eq(proposal_model.proposal_type),
                    circuit_proposal::circuit_hash.eq(proposal_model.circuit_hash),
                    circuit_proposal::requester.eq(proposal_model.requester),
                    circuit_proposal::requester_node_id.eq(proposal_model.requester_node_id),
                ))
                .execute(self.conn)?;
            // Update existing `ProposedCircuit`
            let proposed_circuit_model = ProposedCircuitModel::from(proposal.circuit());
            update(proposed_circuit::table.find(proposal.circuit_id()))
                .set((
                    proposed_circuit::authorization_type
                        .eq(proposed_circuit_model.authorization_type),
                    proposed_circuit::persistence.eq(proposed_circuit_model.persistence),
                    proposed_circuit::durability.eq(proposed_circuit_model.durability),
                    proposed_circuit::routes.eq(proposed_circuit_model.routes),
                    proposed_circuit::circuit_management_type
                        .eq(proposed_circuit_model.circuit_management_type),
                    proposed_circuit::application_metadata
                        .eq(proposed_circuit_model.application_metadata),
                    proposed_circuit::comments.eq(proposed_circuit_model.comments),
                ))
                .execute(self.conn)?;

            // Delete existing data associated with the `CircuitProposal` and `ProposedCircuit`
            let node_ids: Vec<String> = proposed_node::table
                .filter(proposed_node::circuit_id.eq(proposal.circuit_id()))
                .select(proposed_node::node_id)
                .load(self.conn)?;

            delete(
                proposed_node::table.filter(proposed_node::circuit_id.eq(proposal.circuit_id())),
            )
            .execute(self.conn)?;
            delete(
                proposed_node_endpoint::table
                    .filter(proposed_node_endpoint::node_id.eq_any(&node_ids)),
            )
            .execute(self.conn)?;
            delete(
                proposed_service::table
                    .filter(proposed_service::circuit_id.eq(proposal.circuit_id())),
            )
            .execute(self.conn)?;
            delete(
                proposed_service_argument::table
                    .filter(proposed_service_argument::circuit_id.eq(proposal.circuit_id())),
            )
            .execute(self.conn)?;
            delete(vote_record::table.filter(vote_record::circuit_id.eq(proposal.circuit_id())))
                .execute(self.conn)?;

            // Insert the updated info for all of the `CircuitProposal` and `ProposedCircuit`
            // associated data
            // Insert `members` of a `ProposedCircuit`
            let proposed_members: Vec<ProposedNodeModel> = Vec::from(proposal.circuit());
            insert_into(proposed_node::table)
                .values(proposed_members)
                .execute(self.conn)?;
            // Insert the node `endpoints` the proposed `members` of a `ProposedCircuit`
            let proposed_member_endpoints: Vec<ProposedNodeEndpointModel> =
                Vec::from(proposal.circuit());
            insert_into(proposed_node_endpoint::table)
                .values(proposed_member_endpoints)
                .execute(self.conn)?;
            // Insert `roster`, list of `Services` of a `ProposedCircuit`
            let proposed_service: Vec<ProposedServiceModel> = Vec::from(proposal.circuit());
            insert_into(proposed_service::table)
                .values(proposed_service)
                .execute(self.conn)?;
            // Insert `service_arguments` from the `Services` inserted above
            let proposed_service_argument: Vec<ProposedServiceArgumentModel> =
                Vec::from(proposal.circuit());
            insert_into(proposed_service_argument::table)
                .values(proposed_service_argument)
                .execute(self.conn)?;
            // Insert `votes` from the `CircuitProposal`
            let vote_record: Vec<VoteRecordModel> = Vec::from(&proposal);
            insert_into(vote_record::table)
                .values(vote_record)
                .execute(self.conn)?;

            Ok(())
        })
    }
}
