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

//! Provides the " count proposals" operation for the `DieselAdminServiceStore`.

use std::convert::TryFrom;

use diesel::{
    dsl::{count_star, exists},
    prelude::*,
    sql_types::{Binary, Integer, Nullable, SmallInt, Text},
};

use crate::admin::store::{
    diesel::{
        models::{CircuitProposalModel, ProposedCircuitModel},
        schema::{proposed_circuit, proposed_node},
    },
    error::AdminServiceStoreError,
    CircuitPredicate,
};
use crate::error::InternalError;

use super::AdminServiceStoreOperations;

pub(in crate::admin::store::diesel) trait AdminServiceStoreCountProposalsOperation {
    fn count_proposals(
        &self,
        predicates: &[CircuitPredicate],
    ) -> Result<u32, AdminServiceStoreError>;
}

impl<'a, C> AdminServiceStoreCountProposalsOperation for AdminServiceStoreOperations<'a, C>
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
            Integer,
            SmallInt,
        ),
        C::Backend,
    >,
{
    fn count_proposals(
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

        self.conn.transaction::<u32, _, _>(|| {
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

            let count = query.select(count_star()).first::<i64>(self.conn)?;

            u32::try_from(count).map_err(|_| {
                AdminServiceStoreError::InternalError(InternalError::with_message(
                    "The number of proposals is larger than the max u32".to_string(),
                ))
            })
        })
    }
}
