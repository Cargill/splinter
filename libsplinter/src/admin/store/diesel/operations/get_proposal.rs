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

//! Provides the "fetch proposal" operation for the `DieselRegistry`.

use diesel::{
    prelude::*,
    sql_types::{Binary, Integer, Nullable, Text},
};
use std::collections::HashMap;
use std::convert::TryFrom;

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
    AuthorizationType, CircuitProposal, CircuitProposalBuilder, DurabilityType, PersistenceType,
    ProposalType, ProposedCircuitBuilder, ProposedNode, ProposedNodeBuilder, ProposedService,
    ProposedServiceBuilder, RouteType, VoteRecord,
};

pub(in crate::admin::store::diesel) trait AdminServiceStoreFetchProposalOperation {
    fn get_proposal(
        &self,
        proposal_id: &str,
    ) -> Result<Option<CircuitProposal>, AdminServiceStoreError>;
}

impl<'a, C> AdminServiceStoreFetchProposalOperation for AdminServiceStoreOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, C::Backend>,
    CircuitProposalModel: diesel::Queryable<(Text, Text, Text, Binary, Text), C::Backend>,
    ProposedCircuitModel: diesel::Queryable<
        (
            Text,
            Text,
            Text,
            Text,
            Text,
            Text,
            Nullable<Binary>,
            Nullable<Text>,
            Nullable<Text>,
        ),
        C::Backend,
    >,
    VoteRecordModel: diesel::Queryable<(Text, Binary, Text, Text, Integer), C::Backend>,
{
    fn get_proposal(
        &self,
        proposal_id: &str,
    ) -> Result<Option<CircuitProposal>, AdminServiceStoreError> {
        self.conn.transaction::<Option<CircuitProposal>, _, _>(|| {
            let (proposal, proposed_circuit): (CircuitProposalModel, ProposedCircuitModel) =
                // return None if the `circuit_proposal` does not exist
                match circuit_proposal::table
                    // The `circuit_proposal` and `proposed_circuit` have a one-to-one relationhip
                    // which allows for the returned entries to be returned as a pair, and the
                    // `inner_join` allows for the data from each table to be returned in this query.
                    .inner_join(
                        proposed_circuit::table
                            .on(circuit_proposal::circuit_id.eq(proposed_circuit::circuit_id)),
                    )
                    // Filters the entries by the provided `proposal_id`
                    .filter(circuit_proposal::circuit_id.eq(proposal_id))
                    .first::<(CircuitProposalModel, ProposedCircuitModel)>(self.conn)
                    .optional()? {
                    Some((proposal, proposed_circuit)) => (proposal, proposed_circuit),
                    None => return Ok(None),
                };
            // If the proposal exists, we must fetch all associated data
            let mut proposed_node_endpoints: HashMap<String, Vec<ProposedNodeEndpointModel>> =
                HashMap::new();
            let mut nodes: HashMap<String, ProposedNodeModel> = HashMap::new();
            for (node, endpoint) in proposed_node::table
                // As `proposed_node` and `proposed_node_endpoint` have a one-to-many relationship,
                // this join will return all matching entries as there are `proposed_node_endpoint`
                // entries.
                .inner_join(
                    proposed_node_endpoint::table.on(proposed_node::node_id
                        .eq(proposed_node_endpoint::node_id)
                        .and(proposed_node_endpoint::circuit_id.eq(proposed_node::circuit_id))),
                )
                // Filters the entries based on the provided `proposal_id`.
                .filter(proposed_node::circuit_id.eq(&proposal.circuit_id))
                .select((
                    proposed_node::all_columns,
                    proposed_node_endpoint::all_columns,
                ))
                .load::<(ProposedNodeModel, ProposedNodeEndpointModel)>(self.conn)?
            {
                if let Some(endpoint_list) = proposed_node_endpoints.get_mut(&node.node_id) {
                    endpoint_list.push(endpoint);
                } else {
                    proposed_node_endpoints.insert(node.node_id.to_string(), vec![endpoint]);
                }
                if !nodes.contains_key(&node.node_id) {
                    nodes.insert(node.node_id.to_string(), node);
                }
            }

            let mut nodes_vec: Vec<ProposedNodeModel> =
                nodes.into_iter().map(|(_, node)| node).collect();
            nodes_vec.sort_by_key(|node| node.position);

            let built_proposed_nodes: Vec<ProposedNode> = nodes_vec
                .into_iter()
                .map(|node| {
                    let mut builder = ProposedNodeBuilder::new().with_node_id(&node.node_id);
                    if let Some(endpoint_mods) = proposed_node_endpoints.get_mut(&node.node_id) {
                        endpoint_mods.sort_by_key(|endpoint| endpoint.position);
                        let endpoints = endpoint_mods
                            .iter()
                            .map(|endpoint_mod| endpoint_mod.endpoint.to_string())
                            .collect::<Vec<String>>();
                        builder = builder.with_endpoints(&endpoints);
                    }
                    builder
                        .build()
                        .map_err(AdminServiceStoreError::InvalidStateError)
                })
                .collect::<Result<Vec<ProposedNode>, AdminServiceStoreError>>()?;

            // Create HashMap of `service_id` to a `ProposedServiceModel` to collect
            // `ProposedService` information
            let mut proposed_services: HashMap<String, ProposedServiceModel> = HashMap::new();
            // Create HashMap of `service_id` to the associated argument values
            let mut arguments_map: HashMap<String, Vec<ProposedServiceArgumentModel>> =
                HashMap::new();
            // Collect all 'proposed_service' entries and associated data using `inner_join`, as
            // `proposed_service` has a one-to-many relationship to `proposed_service_argument`.
            for (proposed_service, opt_arg) in proposed_service::table
                .filter(proposed_service::circuit_id.eq(&proposal.circuit_id))
                // The `proposed_service` table has a one-to-many relationship with the
                // `proposed_service_argument` table. The `inner_join` will retrieve the
                // `proposed_service` and all `proposed_service_argument` entries with the matching
                // `circuit_id` and `service_id`.
                .inner_join(
                    proposed_service_argument::table.on(proposed_service::circuit_id
                        .eq(proposed_service_argument::circuit_id)
                        .and(
                            proposed_service::service_id.eq(proposed_service_argument::service_id),
                        )),
                )
                // Making the `proposed_service_argument` data `nullable`, removes the requirement
                // for different numbers of each to be returned with, or without an associated
                // entry from the other table.
                .select((
                    proposed_service::all_columns,
                    proposed_service_argument::all_columns.nullable(),
                ))
                .load::<(ProposedServiceModel, Option<ProposedServiceArgumentModel>)>(self.conn)?
            {
                if let Some(arg_model) = opt_arg {
                    if let Some(args) = arguments_map.get_mut(&proposed_service.service_id) {
                        args.push(arg_model);
                    } else {
                        arguments_map
                            .insert(proposed_service.service_id.to_string(), vec![arg_model]);
                    }
                }
                // Insert new `ProposedServiceBuilder` if it does not already exist
                if !proposed_services.contains_key(&proposed_service.service_id) {
                    proposed_services
                        .insert(proposed_service.service_id.to_string(), proposed_service);
                }
            }

            let mut service_vec: Vec<ProposedServiceModel> = proposed_services
                .into_iter()
                .map(|(_, service)| service)
                .collect();
            service_vec.sort_by_key(|service| service.position);
            let built_proposed_services: Vec<ProposedService> = service_vec
                .into_iter()
                .map(|service| {
                    let mut builder = ProposedServiceBuilder::new()
                        .with_service_id(&service.service_id)
                        .with_service_type(&service.service_type)
                        .with_node_id(&service.node_id);

                    if let Some(args) = arguments_map.get_mut(&service.service_id) {
                        args.sort_by_key(|arg| arg.position);
                        builder = builder.with_arguments(
                            &args
                                .iter()
                                .map(|arg| (arg.key.to_string(), arg.value.to_string()))
                                .collect::<Vec<(String, String)>>(),
                        );
                    }
                    builder
                        .build()
                        .map_err(AdminServiceStoreError::InvalidStateError)
                })
                .collect::<Result<Vec<ProposedService>, AdminServiceStoreError>>()?;

            // Retrieve all associated `VoteRecord` entries
            let vote_record: Vec<VoteRecord> = vote_record::table
                .filter(vote_record::circuit_id.eq(&proposal.circuit_id))
                .order(vote_record::position)
                .load::<VoteRecordModel>(self.conn)?
                .into_iter()
                .filter_map(|vote| VoteRecord::try_from(&vote).ok())
                .collect();
            let mut builder = ProposedCircuitBuilder::new()
                .with_circuit_id(&proposal.circuit_id)
                .with_roster(&built_proposed_services)
                .with_members(built_proposed_nodes.as_slice())
                .with_authorization_type(&AuthorizationType::try_from(
                    proposed_circuit.authorization_type,
                )?)
                .with_persistence(&PersistenceType::try_from(proposed_circuit.persistence)?)
                .with_durability(&DurabilityType::try_from(proposed_circuit.durability)?)
                .with_routes(&RouteType::try_from(proposed_circuit.routes)?)
                .with_circuit_management_type(&proposed_circuit.circuit_management_type);

            if let Some(application_metadata) = &proposed_circuit.application_metadata {
                builder = builder.with_application_metadata(&application_metadata);
            }

            if let Some(comments) = &proposed_circuit.comments {
                builder = builder.with_comments(&comments);
            }

            if let Some(display_name) = &proposed_circuit.display_name {
                builder = builder.with_display_name(&display_name)
            }

            let native_proposed_circuit = builder
                .build()
                .map_err(AdminServiceStoreError::InvalidStateError)?;
            Ok(Some(
                CircuitProposalBuilder::new()
                    .with_proposal_type(&ProposalType::try_from(proposal.proposal_type)?)
                    .with_circuit_id(&proposal.circuit_id)
                    .with_circuit_hash(&proposal.circuit_hash)
                    .with_circuit(&native_proposed_circuit)
                    .with_votes(&vote_record)
                    .with_requester(&proposal.requester)
                    .with_requester_node_id(&proposal.requester_node_id)
                    .build()
                    .map_err(AdminServiceStoreError::InvalidStateError)?,
            ))
        })
    }
}
