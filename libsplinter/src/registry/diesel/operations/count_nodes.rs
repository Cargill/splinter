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

//! Provides the "count nodes" operation for the `DieselRegistry`.

use diesel::{dsl::sql_query, prelude::*};

use crate::registry::{
    diesel::{models::Count, schema::splinter_nodes},
    MetadataPredicate, RegistryError,
};

use super::{exists_statements_from_metadata_predicates, RegistryOperations};

pub(in crate::registry::diesel) trait RegistryCountNodesOperation {
    fn count_nodes(&self, predicates: &[MetadataPredicate]) -> Result<u32, RegistryError>;
}

impl<'a, C> RegistryCountNodesOperation for RegistryOperations<'a, C>
where
    C: diesel::Connection,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
{
    fn count_nodes(&self, predicates: &[MetadataPredicate]) -> Result<u32, RegistryError> {
        if predicates.is_empty() {
            // No predicates were specified, just count all nodes
            splinter_nodes::table
                .count()
                // Parse as an i64 here because Diesel knows how to convert a `BigInt` into an i64
                .get_result::<i64>(self.conn)
                .map(|count| count as u32)
                .map_err(|err| {
                    RegistryError::general_error_with_source(
                        "Failed to count all nodes",
                        Box::new(err),
                    )
                })
        } else {
            // With predicates, this query is too complicated for pure Diesel, so a raw SQL
            // query is needed.
            let filters = exists_statements_from_metadata_predicates(predicates);
            sql_query(format!(
                "SELECT COUNT(*) FROM splinter_nodes WHERE {}",
                filters
            ))
            // The `Count` struct is required because the deserialized type for a `sql_query` must
            // implement the `QueryableByName` trait, which a raw `i64` does not.
            .get_result::<Count>(self.conn)
            .map(|count| count.count as u32)
            .map_err(|err| {
                RegistryError::general_error_with_source(
                    "Failed to count nodes matching metadata predicates",
                    Box::new(err),
                )
            })
        }
    }
}
