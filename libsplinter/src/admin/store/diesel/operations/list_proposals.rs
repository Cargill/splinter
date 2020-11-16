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

//! Provides the "list proposals" operation for the `DieselAdminServiceStore`.

use std::collections::HashMap;
use std::convert::TryFrom;

use diesel::{
    dsl::exists,
    prelude::*,
    sql_types::{Binary, Text},
};

use crate::admin::store::{
    diesel::{
        models::{
            CircuitProposalModel, ProposedCircuitModel, ProposedNodeModel,
            ProposedServiceArgumentModel, ProposedServiceModel, VoteRecordModel,
        },
        schema::{
            circuit_proposal, proposed_circuit, proposed_node, proposed_node_endpoint,
            proposed_service, proposed_service_argument, vote_record,
        },
    },
    error::AdminServiceStoreError,
    AuthorizationType, CircuitPredicate, CircuitProposal, CircuitProposalBuilder, DurabilityType,
    PersistenceType, ProposalType, ProposedCircuitBuilder, ProposedNode, ProposedNodeBuilder,
    ProposedService, ProposedServiceBuilder, RouteType, VoteRecord,
};
use crate::error::InvalidStateError;

use super::AdminServiceStoreOperations;

pub(in crate::admin::store::diesel) trait AdminServiceStoreListProposalsOperation {
    fn list_proposals(
        &self,
        predicates: &[CircuitPredicate],
    ) -> Result<Box<dyn ExactSizeIterator<Item = CircuitProposal>>, AdminServiceStoreError>;
}

impl<'a, C> AdminServiceStoreListProposalsOperation for AdminServiceStoreOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    CircuitProposalModel: diesel::Queryable<(Text, Text, Text, Binary, Text), C::Backend>,
    ProposedCircuitModel:
        diesel::Queryable<(Text, Text, Text, Text, Text, Text, Binary, Text), C::Backend>,
    VoteRecordModel: diesel::Queryable<(Text, Binary, Text, Text), C::Backend>,
{
    fn list_proposals(
        &self,
        predicates: &[CircuitPredicate],
    ) -> Result<Box<dyn ExactSizeIterator<Item = CircuitProposal>>, AdminServiceStoreError> {
        // Collect the management types included in the list of `CircuitPredicates`
        let management_types: Vec<String> = predicates
            .iter()
            .filter_map(|pred| match pred {
                CircuitPredicate::ManagementTypeEq(man_type) => Some(man_type.to_string()),
                _ => None,
            })
            .collect::<Vec<String>>();
        // Collects the members included in the list of `CircuitPredicates`
        let members: Vec<String> = predicates
            .iter()
            .filter_map(|pred| match pred {
                CircuitPredicate::MembersInclude(members) => Some(members.to_vec()),
                _ => None,
            })
            .flatten()
            .collect();

        self.conn
            .transaction::<Box<dyn ExactSizeIterator<Item = CircuitProposal>>, _, _>(|| {
                let mut query = proposed_circuit::table
                    .into_boxed()
                    .select(proposed_circuit::all_columns);

                if !members.is_empty() {
                    query = query.filter(exists(
                        // Selects all `proposed_node` entries where the `node_id` is not equal
                        // to any of the members in the circuit predicates
                        proposed_node::table.filter(
                            proposed_node::circuit_id
                                .eq(proposed_circuit::circuit_id)
                                .and(proposed_node::node_id.eq_any(members)),
                        ),
                    ))
                }

                // Selects proposed circuits that match the management types
                if !management_types.is_empty() {
                    query = query
                        .filter(proposed_circuit::circuit_management_type.eq_any(management_types));
                }

                // Collects proposed circuits which match the circuit predicates
                let proposed_circuits: Vec<ProposedCircuitModel> = query
                    .order(proposed_circuit::circuit_id.desc())
                    .load::<ProposedCircuitModel>(self.conn)?;

                // Store circuit IDs separately to make it easier to filter following queries
                let circuit_ids: Vec<&str> = proposed_circuits
                    .iter()
                    .map(|proposed_circuit| proposed_circuit.circuit_id.as_str())
                    .collect();

                let circuit_proposals: HashMap<String, CircuitProposalModel> =
                    circuit_proposal::table
                        .filter(circuit_proposal::circuit_id.eq_any(&circuit_ids))
                        .load::<CircuitProposalModel>(self.conn)?
                        // Once the `CircuitProposalModels` have been
                        // collected,  organize into a HashMap.
                        .into_iter()
                        .map(|proposal| (proposal.circuit_id.to_string(), proposal))
                        .collect();

                let proposal_builders: Vec<(
                    String,
                    (CircuitProposalBuilder, ProposedCircuitBuilder),
                )> = proposed_circuits
                    .into_iter()
                    .map(|proposed_circuit| {
                        let proposal = circuit_proposals
                            .get(&proposed_circuit.circuit_id)
                            .ok_or_else(|| {
                                AdminServiceStoreError::InvalidStateError(
                                    InvalidStateError::with_message(format!(
                                        "Missing proposal for proposed_circuit {}",
                                        proposed_circuit.circuit_id
                                    )),
                                )
                            })?;

                        let proposal_builder = CircuitProposalBuilder::new()
                            .with_proposal_type(&ProposalType::try_from(
                                proposal.proposal_type.to_string(),
                            )?)
                            .with_circuit_id(&proposal.circuit_id)
                            .with_circuit_hash(&proposal.circuit_hash)
                            .with_requester(&proposal.requester)
                            .with_requester_node_id(&proposal.requester_node_id);
                        let proposed_circuit_builder = ProposedCircuitBuilder::new()
                            .with_circuit_id(&proposed_circuit.circuit_id)
                            .with_authorization_type(&AuthorizationType::try_from(
                                proposed_circuit.authorization_type,
                            )?)
                            .with_persistence(&PersistenceType::try_from(
                                proposed_circuit.persistence,
                            )?)
                            .with_durability(&DurabilityType::try_from(
                                proposed_circuit.durability,
                            )?)
                            .with_routes(&RouteType::try_from(proposed_circuit.routes)?)
                            .with_circuit_management_type(&proposed_circuit.circuit_management_type)
                            .with_application_metadata(&proposed_circuit.application_metadata)
                            .with_comments(&proposed_circuit.comments);
                        Ok((
                            proposed_circuit.circuit_id.to_string(),
                            (proposal_builder, proposed_circuit_builder),
                        ))
                    })
                    .collect::<Result<Vec<(_, _)>, AdminServiceStoreError>>()?;

                // Collect `ProposedServices` to apply to the `ProposedCircuit`
                // Create HashMap of (`circuit_id`, `service_id`) to a `ProposedServiceBuilder`
                let mut proposed_services: HashMap<(String, String), ProposedServiceBuilder> =
                    HashMap::new();
                // Create HashMap of (`circuit_id`, `service_id`) to the associated argument values
                let mut arguments_map: HashMap<(String, String), Vec<(String, String)>> =
                    HashMap::new();
                for (proposed_service, opt_arg) in proposed_service::table
                    .left_join(
                        proposed_service_argument::table.on(proposed_service::service_id
                            .eq(proposed_service_argument::service_id)
                            .and(
                                proposed_service_argument::circuit_id
                                    .eq(proposed_service::circuit_id),
                            )),
                    )
                    .select((
                        proposed_service::all_columns,
                        proposed_service_argument::all_columns.nullable(),
                    ))
                    .load::<(ProposedServiceModel, Option<ProposedServiceArgumentModel>)>(
                        self.conn,
                    )?
                {
                    if let Some(arg_model) = opt_arg {
                        if let Some(args) = arguments_map.get_mut(&(
                            proposed_service.circuit_id.to_string(),
                            proposed_service.service_id.to_string(),
                        )) {
                            args.push((arg_model.key.to_string(), arg_model.value.to_string()));
                        } else {
                            arguments_map.insert(
                                (
                                    proposed_service.circuit_id.to_string(),
                                    proposed_service.service_id.to_string(),
                                ),
                                vec![(arg_model.key.to_string(), arg_model.value.to_string())],
                            );
                        }
                    }
                    // Insert new `ProposedServiceBuilder` if it does not already exist
                    proposed_services
                        .entry((
                            proposed_service.circuit_id.to_string(),
                            proposed_service.service_id.to_string(),
                        ))
                        .or_insert_with(|| {
                            ProposedServiceBuilder::new()
                                .with_service_id(&proposed_service.service_id)
                                .with_service_type(&proposed_service.service_type)
                                .with_node_id(&proposed_service.node_id)
                        });
                }
                // Need to collect the `ProposedServices` mapped to `circuit_ids`
                let mut built_proposed_services: HashMap<String, Vec<ProposedService>> =
                    HashMap::new();
                for ((circuit_id, service_id), mut builder) in proposed_services.into_iter() {
                    if let Some(args) =
                        arguments_map.get(&(circuit_id.to_string(), service_id.to_string()))
                    {
                        builder = builder.with_arguments(&args);
                    }
                    let proposed_service = builder
                        .build()
                        .map_err(AdminServiceStoreError::InvalidStateError)?;

                    if let Some(service_list) = built_proposed_services.get_mut(&circuit_id) {
                        service_list.push(proposed_service);
                    } else {
                        built_proposed_services
                            .insert(circuit_id.to_string(), vec![proposed_service]);
                    }
                }
                // Collect `ProposedNodes` and proposed node endpoints
                let mut proposed_nodes: HashMap<(String, String), ProposedNodeBuilder> =
                    HashMap::new();
                for (node, endpoint) in proposed_node::table
                    .inner_join(
                        proposed_node_endpoint::table.on(proposed_node::node_id
                            .eq(proposed_node_endpoint::node_id)
                            .and(proposed_node_endpoint::circuit_id.eq(proposed_node::circuit_id))),
                    )
                    .select((proposed_node::all_columns, proposed_node_endpoint::endpoint))
                    .load::<(ProposedNodeModel, String)>(self.conn)?
                {
                    if let Some(proposed_node) = proposed_nodes
                        .remove(&(node.circuit_id.to_string(), node.node_id.to_string()))
                    {
                        if let Some(mut endpoints) = proposed_node.endpoints() {
                            endpoints.push(endpoint);
                            let proposed_node = proposed_node.with_endpoints(&endpoints);
                            proposed_nodes.insert((node.circuit_id, node.node_id), proposed_node);
                        } else {
                            let proposed_node = proposed_node.with_endpoints(&[endpoint]);
                            proposed_nodes.insert((node.circuit_id, node.node_id), proposed_node);
                        }
                    } else {
                        let proposed_node = ProposedNodeBuilder::new()
                            .with_node_id(&node.node_id)
                            .with_endpoints(&[endpoint]);
                        proposed_nodes.insert((node.circuit_id, node.node_id), proposed_node);
                    }
                }
                let mut built_proposed_nodes: HashMap<String, Vec<ProposedNode>> = HashMap::new();
                for ((circuit_id, _), builder) in proposed_nodes.into_iter() {
                    if let Some(nodes) = built_proposed_nodes.get_mut(&circuit_id) {
                        nodes.push(
                            builder
                                .build()
                                .map_err(AdminServiceStoreError::InvalidStateError)?,
                        )
                    } else {
                        built_proposed_nodes.insert(
                            circuit_id.to_string(),
                            vec![builder
                                .build()
                                .map_err(AdminServiceStoreError::InvalidStateError)?],
                        );
                    }
                }

                // Collect votes to apply to the 'CircuitProposal'
                let mut vote_records: HashMap<String, Vec<VoteRecord>> = HashMap::new();
                for vote in vote_record::table
                    .load::<VoteRecordModel>(self.conn)?
                    .into_iter()
                {
                    if let Some(votes) = vote_records.get_mut(&vote.circuit_id) {
                        votes.push(VoteRecord::try_from(&vote)?);
                    } else {
                        vote_records.insert(
                            vote.circuit_id.to_string(),
                            vec![VoteRecord::try_from(&vote)?],
                        );
                    }
                }

                let mut proposals: Vec<CircuitProposal> = Vec::new();
                for (circuit_id, (mut proposal_builder, mut proposed_circuit_builder)) in
                    proposal_builders
                {
                    if let Some(services) = built_proposed_services.get(&circuit_id) {
                        proposed_circuit_builder = proposed_circuit_builder.with_roster(&services);
                    }
                    if let Some(nodes) = built_proposed_nodes.get(&circuit_id) {
                        proposed_circuit_builder = proposed_circuit_builder.with_members(nodes);
                    }
                    if let Some(votes) = vote_records.get(&circuit_id) {
                        proposal_builder = proposal_builder.with_votes(&votes);
                    }
                    proposals.push(
                        proposal_builder
                            .with_circuit(
                                &proposed_circuit_builder.build().map_err(|err| {
                                    AdminServiceStoreError::InvalidStateError(err)
                                })?,
                            )
                            .build()
                            .map_err(AdminServiceStoreError::InvalidStateError)?,
                    )
                }

                Ok(Box::new(proposals.into_iter()))
            })
    }
}
