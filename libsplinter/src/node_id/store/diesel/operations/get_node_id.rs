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

use diesel::prelude::*;

use crate::node_id::store::{diesel::NodeID, NodeIdStoreError};

use super::NodeIdOperations;

pub trait NodeIdGetOperation {
    fn get_node_id(&self) -> Result<Option<String>, NodeIdStoreError>;
}

impl<'a, C> NodeIdGetOperation for NodeIdOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn get_node_id(&self) -> Result<Option<String>, NodeIdStoreError> {
        use crate::node_id::store::diesel::schema::node_id::dsl::*;
        let result = node_id.first::<NodeID>(self.connection);
        match result {
            Ok(s) => Ok(Some(s.id)),
            Err(diesel::result::Error::NotFound) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }
}
