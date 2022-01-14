// Copyright 2018-2022 Cargill Incorporated
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

//! Provides the "count circuits" operation for the `DieselAdminServiceStore`.

use std::convert::TryFrom;

use diesel::{
    dsl::{count_star, exists},
    prelude::*,
};

use crate::admin::store::{
    diesel::{
        models::CircuitStatusModel,
        schema::{circuit, circuit_member},
    },
    error::AdminServiceStoreError,
    CircuitPredicate,
};
use crate::error::InternalError;

use super::AdminServiceStoreOperations;

pub(in crate::admin::store::diesel) trait AdminServiceStoreCountCircuitsOperation {
    fn count_circuits(
        &self,
        predicates: &[CircuitPredicate],
    ) -> Result<u32, AdminServiceStoreError>;
}

impl<'a, C> AdminServiceStoreCountCircuitsOperation for AdminServiceStoreOperations<'a, C>
where
    C: diesel::Connection,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
{
    fn count_circuits(
        &self,
        predicates: &[CircuitPredicate],
    ) -> Result<u32, AdminServiceStoreError> {
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
        let statuses: Vec<CircuitStatusModel> = predicates
            .iter()
            .filter_map(|pred| match pred {
                CircuitPredicate::CircuitStatus(status) => Some(CircuitStatusModel::from(status)),
                _ => None,
            })
            .collect();
        self.conn.transaction::<u32, _, _>(|| {
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

            if statuses.is_empty() {
                // By default, only display active circuits
                query = query.filter(circuit::circuit_status.eq(CircuitStatusModel::Active));
            } else {
                query = query.filter(
                    // Select only circuits that have the `CircuitStatus` in the predicates
                    circuit::circuit_status.eq_any(statuses),
                );
            }

            let count = query.select(count_star()).first::<i64>(self.conn)?;

            u32::try_from(count).map_err(|_| {
                AdminServiceStoreError::InternalError(InternalError::with_message(
                    "The number of circuits is larger than the max u32".to_string(),
                ))
            })
        })
    }
}
