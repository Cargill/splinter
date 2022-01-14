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

//! Provides [NodeIdStore](super::NodeIdStore) Operations to
//! [NodeIdStore](super::NodeIdStore) implementors.

pub(super) mod get_node_id;
pub(super) mod set_node_id;

pub struct NodeIdOperations<'a, C> {
    connection: &'a C,
}

impl<'a, C> NodeIdOperations<'a, C>
where
    C: diesel::Connection,
{
    /// Constructs new NodeIdOperations struct
    ///
    /// # Arguments
    ///
    ///  * 'connection' - Database connection
    pub fn new(connection: &'a C) -> Self {
        Self { connection }
    }
}
