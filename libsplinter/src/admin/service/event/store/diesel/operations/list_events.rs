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

//! Used by operations to retrieve all `AdminServiceEvent` instances in the database that match
//! the specifie event IDs.

use std::collections::HashMap;
use std::convert::TryFrom;

use diesel::{prelude::*, types::HasSqlType};

use super::AdminServiceEventStoreOperations;

use crate::admin::service::event::{
    store::{
        diesel::{
            models::{
                AdminEventCircuitProposalModel, AdminEventProposedCircuitModel,
                AdminEventProposedNodeEndpointModel, AdminEventProposedNodeModel,
                AdminEventProposedServiceArgumentModel, AdminEventProposedServiceModel,
                AdminEventVoteRecordModel, AdminServiceEventModel,
            },
            schema::{
                admin_event_circuit_proposal, admin_event_proposed_circuit,
                admin_event_proposed_node, admin_event_proposed_node_endpoint,
                admin_event_proposed_service, admin_event_proposed_service_argument,
                admin_event_vote_record, admin_service_event,
            },
        },
        AdminServiceEventStoreError, EventIter,
    },
    AdminServiceEvent,
};
use crate::admin::store::{
    AuthorizationType, CircuitProposalBuilder, DurabilityType, PersistenceType, ProposalType,
    ProposedCircuitBuilder, ProposedNode, ProposedNodeBuilder, ProposedService,
    ProposedServiceBuilder, RouteType, VoteRecord,
};

pub(in crate::admin::service::event::store::diesel) trait AdminServiceEventStoreListEventsOperation
{
    fn list_events(&self, events_id: Vec<i64>) -> Result<EventIter, AdminServiceEventStoreError>;
}

impl<'a, C> AdminServiceEventStoreListEventsOperation for AdminServiceEventStoreOperations<'a, C>
where
    C: diesel::Connection,
    C::Backend: HasSqlType<diesel::sql_types::BigInt>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, C::Backend>,
    Vec<u8>: diesel::deserialize::FromSql<diesel::sql_types::Binary, C::Backend>,
{
    fn list_events(&self, event_ids: Vec<i64>) -> Result<EventIter, AdminServiceEventStoreError> {
        self.conn.transaction::<EventIter, _, _>(|| {
            // List of the events, and the one-to-one models present in the database
            let event_models: Vec<(
                AdminServiceEventModel,
                AdminEventCircuitProposalModel,
                AdminEventProposedCircuitModel,
            )> = admin_service_event::table
                .filter(admin_service_event::id.eq_any(&event_ids))
                .inner_join(
                    admin_event_circuit_proposal::table
                        .on(admin_service_event::id.eq(admin_event_circuit_proposal::event_id)),
                )
                .inner_join(
                    admin_event_proposed_circuit::table
                        .on(admin_service_event::id.eq(admin_event_proposed_circuit::event_id)),
                )
                .load::<(
                    AdminServiceEventModel,
                    AdminEventCircuitProposalModel,
                    AdminEventProposedCircuitModel,
                )>(self.conn)?;
            // Transform previously-retrieved models into builders, keyed to the event ID
            let events_map: HashMap<
                i64,
                (
                    AdminServiceEventModel,
                    CircuitProposalBuilder,
                    ProposedCircuitBuilder,
                ),
            > = event_models
                .into_iter()
                .map(
                    |(event_model, circuit_proposal_model, proposed_circuit_model)| {
                        let proposal_builder = CircuitProposalBuilder::new()
                            .with_proposal_type(&ProposalType::try_from(
                                circuit_proposal_model.proposal_type.to_string(),
                            )?)
                            .with_circuit_id(&circuit_proposal_model.circuit_id)
                            .with_circuit_hash(&circuit_proposal_model.circuit_hash)
                            .with_requester(&circuit_proposal_model.requester)
                            .with_requester_node_id(&circuit_proposal_model.requester_node_id);
                        let mut proposed_circuit_builder = ProposedCircuitBuilder::new()
                            .with_circuit_id(&proposed_circuit_model.circuit_id)
                            .with_authorization_type(&AuthorizationType::try_from(
                                proposed_circuit_model.authorization_type,
                            )?)
                            .with_persistence(&PersistenceType::try_from(
                                proposed_circuit_model.persistence,
                            )?)
                            .with_durability(&DurabilityType::try_from(
                                proposed_circuit_model.durability,
                            )?)
                            .with_routes(&RouteType::try_from(proposed_circuit_model.routes)?)
                            .with_circuit_management_type(
                                &proposed_circuit_model.circuit_management_type,
                            );
                        if let Some(application_metadata) =
                            &proposed_circuit_model.application_metadata
                        {
                            proposed_circuit_builder = proposed_circuit_builder
                                .with_application_metadata(&application_metadata);
                        }

                        if let Some(comments) = &proposed_circuit_model.comments {
                            proposed_circuit_builder =
                                proposed_circuit_builder.with_comments(&comments);
                        }

                        if let Some(display_name) = &proposed_circuit_model.display_name {
                            proposed_circuit_builder =
                                proposed_circuit_builder.with_display_name(&display_name);
                        }

                        Ok((
                            event_model.id,
                            (event_model, proposal_builder, proposed_circuit_builder),
                        ))
                    },
                )
                .collect::<Result<HashMap<i64, (_, _, _)>, AdminServiceEventStoreError>>()?;

            // Collect `ProposedServices` to apply to the `ProposedCircuit`
            // Create HashMap of (`event_id`, `service_id`) to a `IndexedServiceBuilder`
            let mut proposed_services: HashMap<(i64, String), IndexedServiceBuilder> =
                HashMap::new();

            for (proposed_service, opt_arg) in admin_event_proposed_service::table
                .filter(admin_event_proposed_service::event_id.eq_any(&event_ids))
                .left_join(
                    admin_event_proposed_service_argument::table.on(
                        admin_event_proposed_service::event_id
                            .eq(admin_event_proposed_service_argument::event_id)
                            .and(
                                admin_event_proposed_service::service_id
                                    .eq(admin_event_proposed_service_argument::service_id),
                            ),
                    ),
                )
                .select((
                    admin_event_proposed_service::all_columns,
                    admin_event_proposed_service_argument::all_columns.nullable(),
                ))
                .load::<(
                    AdminEventProposedServiceModel,
                    Option<AdminEventProposedServiceArgumentModel>,
                )>(self.conn)?
            {
                if let Some(arg_model) = opt_arg {
                    if let Some(indexed_service) = proposed_services.get_mut(&(
                        proposed_service.event_id,
                        proposed_service.service_id.to_string(),
                    )) {
                        indexed_service.arguments.push(arg_model);
                    } else {
                        // Insert new `ProposedServiceBuilder` if it does not already exist
                        proposed_services
                            .entry((
                                proposed_service.event_id,
                                proposed_service.service_id.to_string(),
                            ))
                            .or_insert_with(|| IndexedServiceBuilder {
                                position: proposed_service.position,
                                arguments: vec![arg_model],
                                builder: ProposedServiceBuilder::new()
                                    .with_service_id(&proposed_service.service_id)
                                    .with_service_type(&proposed_service.service_type)
                                    .with_node_id(&proposed_service.node_id),
                            });
                    }
                }
            }
            // Need to collect the `ProposedServices` mapped to `event_ids`
            let mut built_proposed_services: HashMap<i64, Vec<ProposedService>> = HashMap::new();

            let mut ordered_proposed_services: Vec<((i64, String), IndexedServiceBuilder)> =
                proposed_services.into_iter().collect();
            ordered_proposed_services
                .sort_by_key(|((_, _), indexed_service)| indexed_service.position);
            for ((event_id, _), mut indexed_service) in ordered_proposed_services.into_iter() {
                indexed_service.arguments.sort_by_key(|arg| arg.position);
                indexed_service.builder = indexed_service.builder.with_arguments(
                    &indexed_service
                        .arguments
                        .iter()
                        .map(|arg_mod| (arg_mod.key.to_string(), arg_mod.value.to_string()))
                        .collect::<Vec<(String, String)>>(),
                );

                let proposed_service = indexed_service
                    .builder
                    .build()
                    .map_err(AdminServiceEventStoreError::InvalidStateError)?;

                if let Some(service_list) = built_proposed_services.get_mut(&event_id) {
                    service_list.push(proposed_service);
                } else {
                    built_proposed_services.insert(event_id, vec![proposed_service]);
                }
            }
            // Collect `Nodes` and proposed node endpoints
            let mut proposed_nodes: HashMap<(i64, String), IndexedNodeBuilder> = HashMap::new();
            for (node, endpoint) in admin_event_proposed_node::table
                .filter(admin_event_proposed_node::event_id.eq_any(&event_ids))
                .inner_join(
                    admin_event_proposed_node_endpoint::table.on(
                        admin_event_proposed_node::node_id
                            .eq(admin_event_proposed_node_endpoint::node_id)
                            .and(
                                admin_event_proposed_node_endpoint::event_id
                                    .eq(admin_event_proposed_node::event_id),
                            ),
                    ),
                )
                .select((
                    admin_event_proposed_node::all_columns,
                    admin_event_proposed_node_endpoint::all_columns,
                ))
                .load::<(
                    AdminEventProposedNodeModel,
                    AdminEventProposedNodeEndpointModel,
                )>(self.conn)?
            {
                if let Some(proposed_node) =
                    proposed_nodes.get_mut(&(node.event_id, node.node_id.to_string()))
                {
                    proposed_node.endpoints.push(endpoint);
                } else {
                    let proposed_node = ProposedNodeBuilder::new().with_node_id(&node.node_id);

                    proposed_nodes.insert(
                        (node.event_id, node.node_id),
                        IndexedNodeBuilder {
                            position: node.position,
                            endpoints: vec![endpoint],
                            builder: proposed_node,
                        },
                    );
                }
            }

            let mut ordered_proposed_nodes: Vec<((i64, String), IndexedNodeBuilder)> =
                proposed_nodes.into_iter().collect();
            ordered_proposed_nodes.sort_by_key(|((_, _), indexed_node)| indexed_node.position);

            let mut built_proposed_nodes: HashMap<i64, Vec<ProposedNode>> = HashMap::new();
            for ((event_id, _), mut proposed_node) in ordered_proposed_nodes.into_iter() {
                if let Some(nodes) = built_proposed_nodes.get_mut(&event_id) {
                    proposed_node
                        .endpoints
                        .sort_by_key(|endpoint_mods| endpoint_mods.position);

                    let endpoints = proposed_node
                        .endpoints
                        .iter()
                        .map(|endpoint_mod| endpoint_mod.endpoint.to_string())
                        .collect::<Vec<String>>();
                    nodes.push(
                        proposed_node
                            .builder
                            .with_endpoints(&endpoints)
                            .build()
                            .map_err(AdminServiceEventStoreError::InvalidStateError)?,
                    )
                } else {
                    proposed_node
                        .endpoints
                        .sort_by_key(|endpoint_mods| endpoint_mods.position);

                    let endpoints = proposed_node
                        .endpoints
                        .iter()
                        .map(|endpoint_mod| endpoint_mod.endpoint.to_string())
                        .collect::<Vec<String>>();
                    built_proposed_nodes.insert(
                        event_id,
                        vec![proposed_node
                            .builder
                            .with_endpoints(&endpoints)
                            .build()
                            .map_err(AdminServiceEventStoreError::InvalidStateError)?],
                    );
                }
            }

            // Collect votes to apply to the 'CircuitProposal'
            let mut vote_records: HashMap<i64, Vec<VoteRecord>> = HashMap::new();
            for vote in admin_event_vote_record::table
                .filter(admin_event_vote_record::event_id.eq_any(&event_ids))
                .order(admin_event_vote_record::position)
                .load::<AdminEventVoteRecordModel>(self.conn)?
                .into_iter()
            {
                if let Some(votes) = vote_records.get_mut(&vote.event_id) {
                    votes.push(
                        VoteRecord::try_from(&vote)
                            .map_err(AdminServiceEventStoreError::InvalidStateError)?,
                    );
                } else {
                    vote_records.insert(
                        vote.event_id,
                        vec![VoteRecord::try_from(&vote)
                            .map_err(AdminServiceEventStoreError::InvalidStateError)?],
                    );
                }
            }

            let mut events: Vec<AdminServiceEvent> = Vec::new();
            for (event_id, (event_model, mut proposal_builder, mut proposed_circuit_builder)) in
                events_map
            {
                if let Some(services) = built_proposed_services.get(&event_id) {
                    proposed_circuit_builder = proposed_circuit_builder.with_roster(&services);
                }
                if let Some(nodes) = built_proposed_nodes.get(&event_id) {
                    proposed_circuit_builder = proposed_circuit_builder.with_members(nodes);
                }
                if let Some(votes) = vote_records.get(&event_id) {
                    proposal_builder = proposal_builder.with_votes(&votes);
                }
                let proposal = proposal_builder
                    .with_circuit(
                        &proposed_circuit_builder
                            .build()
                            .map_err(AdminServiceEventStoreError::InvalidStateError)?,
                    )
                    .build()
                    .map_err(AdminServiceEventStoreError::InvalidStateError)?;
                events.push(AdminServiceEvent::try_from((event_model, proposal))?)
            }
            // Ensure the events are returned in a deterministic order, ascending by event ID
            events.sort_by_key(|a| a.event_id);

            Ok(Box::new(events.into_iter()))
        })
    }
}

struct IndexedNodeBuilder {
    position: i32,
    endpoints: Vec<AdminEventProposedNodeEndpointModel>,
    builder: ProposedNodeBuilder,
}

struct IndexedServiceBuilder {
    position: i32,
    arguments: Vec<AdminEventProposedServiceArgumentModel>,
    builder: ProposedServiceBuilder,
}
