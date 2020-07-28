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

//! Provides database operations for the `DieselRegistry`.

pub(super) mod count_nodes;
pub(super) mod delete_node;
pub(super) mod fetch_node;
pub(super) mod has_node;
pub(super) mod insert_node;
pub(super) mod list_nodes;

use crate::registry::MetadataPredicate;

pub struct RegistryOperations<'a, C> {
    conn: &'a C,
}

impl<'a, C: diesel::Connection> RegistryOperations<'a, C> {
    pub fn new(conn: &'a C) -> Self {
        RegistryOperations { conn }
    }
}

/// Generates a string with a series of `EXISTS` SQL statements from a list of node metadata
/// predicates. Filtering on node metadata is too complicated for pure Diesel, so raw SQL queries
/// are needed.
///
/// This function assumes that the resulting statements will be used with a
/// `SELECT _ FROM splinter_nodes` query. Each `EXISTS` statement checks for the existence of a
/// matching metadata value in the `splinter_nodes_metadata` table.
fn exists_statements_from_metadata_predicates(predicates: &[MetadataPredicate]) -> String {
    predicates
        .iter()
        .map(|predicate| {
            match predicate {
                MetadataPredicate::Eq(key, val) => format!(
                    "EXISTS (SELECT * FROM splinter_nodes_metadata WHERE \
                     splinter_nodes_metadata.identity = splinter_nodes.identity \
                     AND splinter_nodes_metadata.key = {} \
                     AND splinter_nodes_metadata.value = {})",
                    key, val
                ),
                MetadataPredicate::Ne(key, val) => {
                    // If the metadata key is not set for a node, the predicate is
                    // satisfied
                    format!(
                        "NOT EXISTS (SELECT * FROM splinter_nodes_metadata WHERE \
                         splinter_nodes_metadata.identity = splinter_nodes.identity \
                         AND splinter_nodes_metadata.key = {0}) \
                         OR EXISTS (SELECT * FROM splinter_nodes_metadata WHERE \
                         splinter_nodes_metadata.identity = splinter_nodes.identity \
                         AND splinter_nodes_metadata.key = {} \
                         AND splinter_nodes_metadata.value <> {})",
                        key, val
                    )
                }
                MetadataPredicate::Gt(key, val) => format!(
                    "EXISTS (SELECT * FROM splinter_nodes_metadata WHERE \
                     splinter_nodes_metadata.identity = splinter_nodes.identity \
                     AND splinter_nodes_metadata.key = {} \
                     AND splinter_nodes_metadata.value > {})",
                    key, val
                ),
                MetadataPredicate::Ge(key, val) => format!(
                    "EXISTS (SELECT * FROM splinter_nodes_metadata WHERE \
                     splinter_nodes_metadata.identity = splinter_nodes.identity \
                     AND splinter_nodes_metadata.key = {} \
                     AND splinter_nodes_metadata.value >= {})",
                    key, val
                ),
                MetadataPredicate::Lt(key, val) => format!(
                    "EXISTS (SELECT * FROM splinter_nodes_metadata WHERE \
                     splinter_nodes_metadata.identity = splinter_nodes.identity \
                     AND splinter_nodes_metadata.key = {} \
                     AND splinter_nodes_metadata.value < {})",
                    key, val
                ),
                MetadataPredicate::Le(key, val) => format!(
                    "EXISTS (SELECT * FROM splinter_nodes_metadata WHERE \
                     splinter_nodes_metadata.identity = splinter_nodes.identity \
                     AND splinter_nodes_metadata.key = {} \
                     AND splinter_nodes_metadata.value <= {})",
                    key, val
                ),
            }
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}
