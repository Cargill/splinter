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

//! Provides the "has node" operation for the `DieselRegistry`.

use diesel::prelude::*;

use crate::registry::{
    diesel::{models::NodesModel, schema::splinter_nodes},
    RegistryError,
};

use super::RegistryOperations;

pub(in crate::registry::diesel) trait RegistryHasNodeOperation {
    fn has_node(&self, identity: &str) -> Result<bool, RegistryError>;
}

impl<'a, C> RegistryHasNodeOperation for RegistryOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn has_node(&self, identity: &str) -> Result<bool, RegistryError> {
        splinter_nodes::table
            .find(identity)
            .first::<NodesModel>(self.conn)
            .optional()
            .map(|opt| opt.is_some())
            .map_err(|err| {
                RegistryError::general_error_with_source(
                    "Failed to check if node exists",
                    Box::new(err),
                )
            })
    }
}
