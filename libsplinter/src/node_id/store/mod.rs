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

#[cfg(feature = "diesel")]
pub mod diesel;
pub mod error;
pub mod file;

use error::NodeIdStoreError;

/// Trait for interacting with the instances node_id.
pub trait NodeIdStore {
    /// Gets node_id for the instance
    fn get_node_id(&self) -> Result<Option<String>, NodeIdStoreError>;

    /// Sets node_id for the instance
    ///
    /// # Arguments
    ///
    /// * `node_id` - the desired node_id
    fn set_node_id(&self, node_id: String) -> Result<(), NodeIdStoreError>;
}
