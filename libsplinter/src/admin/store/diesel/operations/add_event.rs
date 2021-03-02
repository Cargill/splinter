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

//! Provides the "add event" operation for the `DieselAdminServiceStore`.

use std::convert::TryFrom;

use diesel::{dsl::insert_into, prelude::*};

use super::AdminServiceStoreOperations;

use crate::admin::service::messages;
#[cfg(feature = "sqlite")]
use crate::admin::store::diesel::models::AdminServiceEventModel;
use crate::admin::store::{
    diesel::{
        models::{
            AdminEventCircuitProposalModel, AdminEventProposedCircuitModel,
            AdminEventProposedNodeEndpointModel, AdminEventProposedNodeModel,
            AdminEventProposedServiceArgumentModel, AdminEventProposedServiceModel,
            AdminEventVoteRecordModel, NewAdminServiceEventModel,
        },
        schema::{
            admin_event_circuit_proposal, admin_event_proposed_circuit, admin_event_proposed_node,
            admin_event_proposed_node_endpoint, admin_event_proposed_service,
            admin_event_proposed_service_argument, admin_event_vote_record, admin_service_event,
        },
    },
    AdminServiceEvent, AdminServiceStoreError,
};

use crate::error::{ConstraintViolationError, ConstraintViolationType};

pub(in crate::admin::store::diesel) trait AdminServiceStoreAddEventOperation {
    fn add_event(
        &self,
        event: messages::AdminServiceEvent,
    ) -> Result<AdminServiceEvent, AdminServiceStoreError>;
}

#[cfg(feature = "postgres")]
impl<'a> AdminServiceStoreAddEventOperation
    for AdminServiceStoreOperations<'a, diesel::pg::PgConnection>
{
    fn add_event(
        &self,
        event: messages::AdminServiceEvent,
    ) -> Result<AdminServiceEvent, AdminServiceStoreError> {
        self.conn.transaction::<AdminServiceEvent, _, _>(|| {
            // Create a `NewAdminServiceEventModel` from the event
            let new_event: NewAdminServiceEventModel = NewAdminServiceEventModel::from(&event);
            // This creates the initial event entry, returning the ID from the inserted row
            // to be used to correlate the other `admin_event_*` entries to this event.
            let event_id: i64 = insert_into(admin_service_event::table)
                .values(new_event)
                .returning(admin_service_event::id)
                .get_result(self.conn)?;
            // Saving the event's proposal to build the required models.
            let proposal = event.proposal().clone();

            // Check if an `CircuitProposal` already exists with the given `event_id`
            if admin_event_circuit_proposal::table
                .filter(admin_event_circuit_proposal::event_id.eq(event_id))
                .first::<AdminEventCircuitProposalModel>(self.conn)
                .optional()?
                .is_some()
            {
                return Err(AdminServiceStoreError::ConstraintViolationError(
                    ConstraintViolationError::with_violation_type(ConstraintViolationType::Unique),
                ));
            }
            // Insert the database model of the admin event's `CircuitProposal`
            let proposal_model = AdminEventCircuitProposalModel::from((event_id, &proposal));
            insert_into(admin_event_circuit_proposal::table)
                .values(proposal_model)
                .execute(self.conn)?;
            // Insert `ProposedCircuitModel`, representing the `create_circuit` of an admin event's
            // `CircuitProposal`
            let proposed_circuit_model =
                AdminEventProposedCircuitModel::from((event_id, &proposal.circuit));
            insert_into(admin_event_proposed_circuit::table)
                .values(proposed_circuit_model)
                .execute(self.conn)?;
            // Insert `members` of an admin event's `CreateCircuit`, represented by the
            // `AdminEventProposedCircuitModel`
            let proposed_members: Vec<AdminEventProposedNodeModel> =
                AdminEventProposedNodeModel::list_from_proposal_with_id(event_id, &proposal)?;
            insert_into(admin_event_proposed_node::table)
                .values(proposed_members)
                .execute(self.conn)?;
            // Insert the node `endpoints` and the proposed `members` of an admin event's
            // `CreateCircuit`, represented by the `AdminEventProposedCircuitModel`
            let proposed_member_endpoints: Vec<AdminEventProposedNodeEndpointModel> =
                AdminEventProposedNodeEndpointModel::list_from_proposal_with_id(
                    event_id, &proposal,
                )?;
            insert_into(admin_event_proposed_node_endpoint::table)
                .values(proposed_member_endpoints)
                .execute(self.conn)?;
            // Insert `roster`, list of `Services` of an admin event's `CreateCircuit`,
            // represented by the `AdminEventProposedCircuitModel`
            let proposed_services: Vec<AdminEventProposedServiceModel> =
                AdminEventProposedServiceModel::list_from_proposal_with_id(event_id, &proposal)?;
            insert_into(admin_event_proposed_service::table)
                .values(proposed_services)
                .execute(self.conn)?;
            // Insert `service_arguments` from the `Services` inserted above
            let proposed_service_arguments: Vec<AdminEventProposedServiceArgumentModel> =
                AdminEventProposedServiceArgumentModel::list_from_proposal_with_id(
                    event_id, &proposal,
                )?;
            insert_into(admin_event_proposed_service_argument::table)
                .values(proposed_service_arguments)
                .execute(self.conn)?;
            // Insert `votes` from the `CircuitProposal`
            let vote_records: Vec<AdminEventVoteRecordModel> =
                AdminEventVoteRecordModel::list_from_proposal_with_id(event_id, &proposal)?;
            insert_into(admin_event_vote_record::table)
                .values(vote_records)
                .execute(self.conn)?;

            AdminServiceEvent::try_from((event_id, &event))
                .map_err(AdminServiceStoreError::InvalidStateError)
        })
    }
}

#[cfg(feature = "sqlite")]
impl<'a> AdminServiceStoreAddEventOperation
    for AdminServiceStoreOperations<'a, diesel::sqlite::SqliteConnection>
{
    fn add_event(
        &self,
        event: messages::AdminServiceEvent,
    ) -> Result<AdminServiceEvent, AdminServiceStoreError> {
        self.conn.transaction::<AdminServiceEvent, _, _>(|| {
            // Create a `NewAdminServiceEventModel` from the event
            let new_event: NewAdminServiceEventModel = NewAdminServiceEventModel::from(&event);
            // This creates the initial event entry, returning the ID from the inserted row
            // to be used to correlate the other `admin_event_*` entries to this event.
            insert_into(admin_service_event::table)
                .values(new_event)
                .execute(self.conn)?;
            // Retrieving the previously inserted event to get the autoincremented ID, used to
            // associate the other database entries to this event.
            let event_id: i64 = admin_service_event::table
                .order(admin_service_event::id.desc())
                .first::<AdminServiceEventModel>(self.conn)?
                .id;

            // Saving the event's proposal to build the required models.
            let proposal = event.proposal().clone();

            // Check if an `CircuitProposal` already exists with the given `event_id`
            if admin_event_circuit_proposal::table
                .filter(admin_event_circuit_proposal::event_id.eq(event_id))
                .first::<AdminEventCircuitProposalModel>(self.conn)
                .optional()?
                .is_some()
            {
                return Err(AdminServiceStoreError::ConstraintViolationError(
                    ConstraintViolationError::with_violation_type(ConstraintViolationType::Unique),
                ));
            }
            // Insert the database model of the admin event's `CircuitProposal`
            let proposal_model = AdminEventCircuitProposalModel::from((event_id, &proposal));
            insert_into(admin_event_circuit_proposal::table)
                .values(proposal_model)
                .execute(self.conn)?;
            // Insert `ProposedCircuitModel`, representing the `create_circuit` of an admin event's
            // `CircuitProposal`
            let proposed_circuit_model =
                AdminEventProposedCircuitModel::from((event_id, &proposal.circuit));
            insert_into(admin_event_proposed_circuit::table)
                .values(proposed_circuit_model)
                .execute(self.conn)?;
            // Insert `members` of an admin event's `CreateCircuit`, represented by the
            // `AdminEventProposedCircuitModel`
            let proposed_members: Vec<AdminEventProposedNodeModel> =
                AdminEventProposedNodeModel::list_from_proposal_with_id(event_id, &proposal)?;
            insert_into(admin_event_proposed_node::table)
                .values(proposed_members)
                .execute(self.conn)?;
            // Insert the node `endpoints` and the proposed `members` of an admin event's
            // `CreateCircuit`, represented by the `AdminEventProposedCircuitModel`
            let proposed_member_endpoints: Vec<AdminEventProposedNodeEndpointModel> =
                AdminEventProposedNodeEndpointModel::list_from_proposal_with_id(
                    event_id, &proposal,
                )?;
            insert_into(admin_event_proposed_node_endpoint::table)
                .values(proposed_member_endpoints)
                .execute(self.conn)?;
            // Insert `roster`, list of `Services` of an admin event's `CreateCircuit`,
            // represented by the `AdminEventProposedCircuitModel`
            let proposed_services: Vec<AdminEventProposedServiceModel> =
                AdminEventProposedServiceModel::list_from_proposal_with_id(event_id, &proposal)?;
            insert_into(admin_event_proposed_service::table)
                .values(proposed_services)
                .execute(self.conn)?;
            // Insert `service_arguments` from the `Services` inserted above
            let proposed_service_arguments: Vec<AdminEventProposedServiceArgumentModel> =
                AdminEventProposedServiceArgumentModel::list_from_proposal_with_id(
                    event_id, &proposal,
                )?;
            insert_into(admin_event_proposed_service_argument::table)
                .values(proposed_service_arguments)
                .execute(self.conn)?;
            // Insert `votes` from the `CircuitProposal`
            let vote_records: Vec<AdminEventVoteRecordModel> =
                AdminEventVoteRecordModel::list_from_proposal_with_id(event_id, &proposal)?;
            insert_into(admin_event_vote_record::table)
                .values(vote_records)
                .execute(self.conn)?;

            AdminServiceEvent::try_from((event_id, &event))
                .map_err(AdminServiceStoreError::InvalidStateError)
        })
    }
}
