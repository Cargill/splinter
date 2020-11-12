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

use diesel::{dsl::exists, prelude::*};

use crate::admin::store::{
    diesel::{
        models::{CircuitMemberModel, CircuitModel, ServiceArgumentModel, ServiceModel},
        schema::{circuit, circuit_member, service, service_argument},
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
            .transaction::<Box<dyn ExactSizeIterator<Item = Circuit>>, _, _>(|| {
                // Collects circuits which match the circuit predicates
                let mut query = circuit::table.into_boxed().select(circuit::all_columns);

                if !management_types.is_empty() {
                    query = query.filter(circuit::circuit_management_type.eq_any(management_types));
                }

                if !members.is_empty() {
                    query = query.filter(exists(
                        // Selects all `circuit_member` entries where the `node_id` is equal
                        // to any of the members in the circuit predicates
                        circuit_member::table.filter(
                            circuit_member::circuit_id
                                .eq(circuit::circuit_id)
                                .and(circuit_member::node_id.eq_any(members)),
                        ),
                    ));
                }

                let circuits: Vec<CircuitModel> = query
                    .order(circuit::circuit_id.desc())
                    .load::<CircuitModel>(self.conn)?;

                // Store circuit IDs separately to make it easier to filter following queries
                let circuit_ids: Vec<&str> = circuits
                    .iter()
                    .map(|circuit| circuit.circuit_id.as_str())
                    .collect();

                // Collect the `Circuit` members and put them in a HashMap to associate the list
                // of `node_ids` to the `circuit_id`
                let mut circuit_members: HashMap<String, Vec<String>> = HashMap::new();
                for member in circuit_member::table
                    .filter(circuit_member::circuit_id.eq_any(&circuit_ids))
                    .load::<CircuitMemberModel>(self.conn)?
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
                // Collects all `service` and `service_argument` entries using an inner_join on the
                // `service_id`, since the relationship between `service` and `service_argument` is
                // one-to-many. Adding the models retrieved from the database backend to HashMaps
                // removed the duplicate `service` entries collected, and also makes it simpler
                // to build each `Service` later on.
                for (service, opt_arg) in service::table
                    // Filters the services based on the circuit_ids collected based on the circuits
                    // which matched the predicates.
                    .filter(service::circuit_id.eq_any(&circuit_ids))
                    // Joins a `service_argument` entry to a `service` entry, based on `service_id`.
                    .left_join(
                        service_argument::table.on(service::service_id
                            .eq(service_argument::service_id)
                            .and(service_argument::circuit_id.eq(service::circuit_id))),
                    )
                    // Collects all data from the `service` entry, and the pertinent data from the
                    // `service_argument` entry.
                    // Making `service_argument` nullable is required to return all matching
                    // records since the relationship with services is one-to-many for each.
                    .select((
                        service::all_columns,
                        service_argument::all_columns.nullable(),
                    ))
                    .load::<(ServiceModel, Option<ServiceArgumentModel>)>(self.conn)?
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
                                .with_node_id(&service.node_id)
                        });
                }
                // Collect the `Services` mapped to `circuit_ids` after adding any
                // `service_arguments` to the `ServiceBuilder`.
                let mut built_services: HashMap<String, Vec<Service>> = HashMap::new();
                for ((circuit_id, service_id), mut builder) in services.into_iter() {
                    if let Some(args) =
                        arguments_map.get(&(circuit_id.to_string(), service_id.to_string()))
                    {
                        builder = builder.with_arguments(&args);
                    }
                    let service = builder
                        .build()
                        .map_err(AdminServiceStoreError::InvalidStateError)?;

                    if let Some(service_list) = built_services.get_mut(&circuit_id) {
                        service_list.push(service);
                    } else {
                        built_services.insert(circuit_id.to_string(), vec![service]);
                    }
                }

                let mut ret_circuits: Vec<Circuit> = Vec::new();
                for model in circuits {
                    let mut circuit_builder = CircuitBuilder::new()
                        .with_circuit_id(&model.circuit_id)
                        .with_authorization_type(&AuthorizationType::try_from(
                            model.authorization_type,
                        )?)
                        .with_persistence(&PersistenceType::try_from(model.persistence)?)
                        .with_durability(&DurabilityType::try_from(model.durability)?)
                        .with_routes(&RouteType::try_from(model.routes)?)
                        .with_circuit_management_type(&model.circuit_management_type);

                    if let Some(members) = circuit_members.get(&model.circuit_id) {
                        circuit_builder = circuit_builder.with_members(&members);
                    }
                    if let Some(services) = built_services.get(&model.circuit_id) {
                        circuit_builder = circuit_builder.with_roster(&services);
                    }

                    ret_circuits.push(
                        circuit_builder
                            .build()
                            .map_err(AdminServiceStoreError::InvalidStateError)?,
                    );
                }

                Ok(Box::new(ret_circuits.into_iter()))
            })
    }
}
