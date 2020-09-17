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

//! Provides the "list circuits" operation for the `DieselAdminServiceStore`.

use std::collections::HashMap;
use std::convert::TryFrom;

use diesel::{
    dsl::{exists, not},
    prelude::*,
};

use crate::admin::store::{
    diesel::{
        models::{CircuitMemberModel, CircuitModel, ServiceArgumentModel, ServiceModel},
        schema::{circuit, circuit_member, service, service_allowed_node, service_argument},
    },
    error::AdminServiceStoreError,
    AuthorizationType, Circuit, CircuitBuilder, CircuitPredicate, DurabilityType, PersistenceType,
    RouteType, Service, ServiceBuilder,
};

use super::AdminServiceStoreOperations;

pub(in crate::admin::store::diesel) trait AdminServiceStoreListCircuitsOperation {
    fn list_circuits(
        &self,
        predicates: &[CircuitPredicate],
    ) -> Result<Box<dyn ExactSizeIterator<Item = Circuit>>, AdminServiceStoreError>;
}

impl<'a, C> AdminServiceStoreListCircuitsOperation for AdminServiceStoreOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
{
    fn list_circuits(
        &self,
        predicates: &[CircuitPredicate],
    ) -> Result<Box<dyn ExactSizeIterator<Item = Circuit>>, AdminServiceStoreError> {
        // Collect the management types included in the list of `CircuitPredicates`
        let management_types: Vec<String> = predicates
            .iter()
            .filter_map(|pred| match pred {
                CircuitPredicate::ManagmentTypeEq(man_type) => Some(man_type.to_string()),
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
            .transaction::<Box<dyn ExactSizeIterator<Item = Circuit>>, _, _>(|| {
                // Collects circuits which match the circuit predicates
                let circuits: HashMap<String, CircuitModel> = circuit::table
                    // Filters based on the circuit's management type
                    .filter(circuit::circuit_management_type.eq_any(management_types))
                    // Circuits are filtered by where there doesn't exist any `circuit_member` entries that
                    // have a matching circuit_id value and have a node_id field that does not equal
                    // any of the IDs collected from the `CircuitPredicates`.
                    .filter(not(exists(
                        // Selects all `circuit_member` entries where the `node_id` is not equal
                        // to any of the members in the circuit predicates
                        circuit_member::table.filter(
                            circuit_member::circuit_id
                                .eq(circuit::circuit_id)
                                .and(circuit_member::node_id.ne_all(members)),
                        ),
                    )))
                    .load::<CircuitModel>(self.conn)
                    .map_err(|err| AdminServiceStoreError::QueryError {
                        context: String::from("Unable to load Circuit information"),
                        source: Box::new(err),
                    })?
                    // Once the `CircuitModels` have been collected, organize into a HashMap
                    .into_iter()
                    .map(|model| (model.circuit_id.to_string(), model))
                    .collect();
                // Store circuit IDs separately to make it easier to filter following queries
                let circuit_ids: Vec<String> = circuits.keys().cloned().collect();

                // Collect the `Circuit` members and put them in a HashMap to associate the list
                // of `node_ids` to the `circuit_id`
                let mut circuit_members: HashMap<String, Vec<String>> = HashMap::new();
                for member in circuit_member::table
                    .filter(circuit_member::circuit_id.eq_any(&circuit_ids))
                    .load::<CircuitMemberModel>(self.conn)
                    .map_err(|err| AdminServiceStoreError::QueryError {
                        context: String::from("Unable to load Circuit member information"),
                        source: Box::new(err),
                    })?
                {
                    if let Some(members) = circuit_members.get_mut(&member.circuit_id) {
                        members.push(member.node_id.to_string());
                    } else {
                        circuit_members.insert(
                            member.circuit_id.to_string(),
                            vec![member.node_id.to_string()],
                        );
                    }
                }

                // Create HashMap of (`circuit_id`, ` service_id`) to a `ServiceBuilder`
                let mut services: HashMap<(String, String), ServiceBuilder> = HashMap::new();
                // Create HashMap of (`circuit_id`, `service_id`) to the associated argument values
                let mut arguments_map: HashMap<(String, String), Vec<(String, String)>> =
                    HashMap::new();
                // Create HashMap of (`circuit_id`, `service_id`) to the associated allowed nodes
                let mut allowed_nodes_map: HashMap<(String, String), Vec<String>> = HashMap::new();
                // Collects all `service`, `service_argument`, and `service_allowed_node` entries
                // using an inner_join on the `service_id`, since the relationship between `service`
                // and `service_argument` and between `service` and `service_allowed_node` is one-
                // to-many. Adding the models retrieved from the database backend to HashMaps
                // removed the duplicate `service` entries collected, and also makes it simpler
                // to build each `Service` later on.
                for (service, opt_arg, opt_allowed_node) in service::table
                    // Filters the services based on the circuit_ids collected based on the circuits
                    // which matched the predicates.
                    .filter(service::circuit_id.eq_any(&circuit_ids))
                    // Joins a `service_argument` entry to a `service` entry, based on `service_id`.
                    .inner_join(
                        service_argument::table
                            .on(service::service_id.eq(service_argument::service_id)),
                    )
                    // Joins a `service_allowed_node` entry to a `service` entry, based on
                    // `service_id`.
                    .inner_join(
                        service_allowed_node::table
                            .on(service::service_id.eq(service_allowed_node::service_id)),
                    )
                    // Collects all data from the `service` entry, and the pertinent data from the
                    // `service_argument` and `service_allowed_node` entry.
                    // Making `service_argument` and `service_allowed_node` nullable is required
                    // to return all matching records since the relationship with services is
                    // one-to-many for each.
                    .select((
                        service::all_columns,
                        service_argument::all_columns.nullable(),
                        service_allowed_node::allowed_node.nullable(),
                    ))
                    .load::<(ServiceModel, Option<ServiceArgumentModel>, Option<String>)>(self.conn)
                    .map_err(|err| AdminServiceStoreError::QueryError {
                        context: String::from("Unable to load Service information"),
                        source: Box::new(err),
                    })?
                {
                    if let Some(arg_model) = opt_arg {
                        if let Some(args) = arguments_map.get_mut(&(
                            service.circuit_id.to_string(),
                            service.service_id.to_string(),
                        )) {
                            args.push((arg_model.key.to_string(), arg_model.value.to_string()));
                        } else {
                            arguments_map.insert(
                                (
                                    service.circuit_id.to_string(),
                                    service.service_id.to_string(),
                                ),
                                vec![(arg_model.key.to_string(), arg_model.value.to_string())],
                            );
                        }
                    }
                    if let Some(allowed_node) = opt_allowed_node {
                        if let Some(list) = allowed_nodes_map.get_mut(&(
                            service.circuit_id.to_string(),
                            service.service_id.to_string(),
                        )) {
                            list.push(allowed_node.to_string());
                        } else {
                            allowed_nodes_map.insert(
                                (
                                    service.circuit_id.to_string(),
                                    service.service_id.to_string(),
                                ),
                                vec![allowed_node.to_string()],
                            );
                        }
                    }
                    // Insert new `ServiceBuilder` if it does not already exist
                    services
                        .entry((
                            service.circuit_id.to_string(),
                            service.service_id.to_string(),
                        ))
                        .or_insert_with(|| {
                            ServiceBuilder::new()
                                .with_service_id(&service.service_id)
                                .with_service_type(&service.service_type)
                        });
                }
                // Collect the `Services` mapped to `circuit_ids` after adding any `service_arguments`
                // and `service_allowed_nodes` to the `ServiceBuilder`.
                let mut built_services: HashMap<String, Vec<Service>> = HashMap::new();
                for ((circuit_id, service_id), mut builder) in services.into_iter() {
                    if let Some(args) =
                        arguments_map.get(&(circuit_id.to_string(), service_id.to_string()))
                    {
                        builder = builder.with_arguments(&args);
                    }
                    if let Some(allowed_nodes) =
                        allowed_nodes_map.get(&(circuit_id.to_string(), service_id.to_string()))
                    {
                        builder = builder.with_allowed_nodes(&allowed_nodes);
                    }
                    let service =
                        builder
                            .build()
                            .map_err(|err| AdminServiceStoreError::StorageError {
                                context: String::from("Unable to build Service"),
                                source: Some(Box::new(err)),
                            })?;
                    if let Some(service_list) = built_services.get_mut(&circuit_id) {
                        service_list.push(service);
                    } else {
                        built_services.insert(circuit_id.to_string(), vec![service]);
                    }
                }

                let mut ret_circuits: Vec<Circuit> = Vec::new();
                for (id, model) in circuits {
                    let mut circuit_builder = CircuitBuilder::new()
                        .with_circuit_id(&model.circuit_id)
                        .with_auth(&AuthorizationType::try_from(model.auth)?)
                        .with_persistence(&PersistenceType::try_from(model.persistence)?)
                        .with_durability(&DurabilityType::try_from(model.durability)?)
                        .with_routes(&RouteType::try_from(model.routes)?);

                    if let Some(members) = circuit_members.get(&id) {
                        circuit_builder = circuit_builder.with_members(&members);
                    }
                    if let Some(services) = built_services.get(&id) {
                        circuit_builder = circuit_builder.with_roster(&services);
                    }

                    ret_circuits.push(circuit_builder.build().map_err(|err| {
                        AdminServiceStoreError::OperationError {
                            context: String::from("Unable to build Circuit"),
                            source: Some(Box::new(err)),
                        }
                    })?);
                }

                Ok(Box::new(ret_circuits.into_iter()))
            })
    }
}
