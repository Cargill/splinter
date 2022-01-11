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

use diesel::insert_into;
use diesel::prelude::*;

use crate::node_id::store::{diesel::NodeID, NodeIdStoreError};

use super::{get_node_id::NodeIdGetOperation, NodeIdOperations};

pub trait NodeIdSetOperation {
    fn set_node_id(&self, new_id: String) -> Result<(), NodeIdStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> NodeIdSetOperation for NodeIdOperations<'a, diesel::sqlite::SqliteConnection> {
    fn set_node_id(&self, new_id: String) -> Result<(), NodeIdStoreError> {
        use super::super::schema::node_id::dsl::*;
        self.connection.transaction(|| match self.get_node_id() {
            Ok(Some(db_id)) => diesel::update(node_id.find(db_id))
                .set(id.eq(new_id))
                .execute(self.connection)
                .map(|_| ())
                .map_err(|e| e.into()),
            Ok(None) => insert_into(node_id)
                .values(NodeID { id: new_id })
                .execute(self.connection)
                .map(|_| ())
                .map_err(|e| e.into()),
            Err(e) => Err(e),
        })
    }
}

#[cfg(feature = "postgres")]
impl<'a> NodeIdSetOperation for NodeIdOperations<'a, diesel::pg::PgConnection> {
    fn set_node_id(&self, new_id: String) -> Result<(), NodeIdStoreError> {
        use super::super::schema::node_id::dsl::*;
        self.connection.transaction(|| match self.get_node_id() {
            Ok(Some(db_id)) => diesel::update(node_id.find(db_id))
                .set(id.eq(new_id))
                .execute(self.connection)
                .map(|_| ())
                .map_err(|e| e.into()),
            Ok(None) => insert_into(node_id)
                .values(NodeID { id: new_id })
                .execute(self.connection)
                .map(|_| ())
                .map_err(|e| e.into()),
            Err(e) => Err(e),
        })
    }
}
