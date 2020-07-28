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

//! Provides the "delete node" operation for the `DieselRegistry`.

use diesel::{dsl::delete, prelude::*};

use crate::registry::{diesel::schema::splinter_nodes, Node, RegistryError};

use super::{fetch_node::RegistryFetchNodeOperation, RegistryOperations};

pub(in crate::registry::diesel) trait RegistryDeleteNodeOperation {
    fn delete_node(&self, identity: &str) -> Result<Option<Node>, RegistryError>;
}

impl<'a, C> RegistryDeleteNodeOperation for RegistryOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn delete_node(&self, identity: &str) -> Result<Option<Node>, RegistryError> {
        self.fetch_node(identity).and_then(|node| {
            delete(splinter_nodes::table.find(identity))
                .execute(self.conn)
                .map_err(|err| {
                    RegistryError::general_error_with_source("Failed to delete node", Box::new(err))
                })?;
            Ok(node)
        })
    }
}
