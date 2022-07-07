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

//! Traits and implementations useful for communicating with the registry as a client.

use std::fmt;
use std::fmt::Write as _;

#[cfg(feature = "registry-client-reqwest")]
mod reqwest;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::InternalError;

pub use self::reqwest::ReqwestRegistryClient;

pub trait RegistryClient {
    /// Adds a node to the registry.
    ///
    /// # Arguments
    ///
    /// * `node` - The node to be added
    fn add_node(&self, node: &RegistryNode) -> Result<(), InternalError>;

    /// Retrieves a node from the registry.
    ///
    /// # Arguments
    ///
    /// * `identity` - The id of the node to be retrieved
    fn get_node(&self, identity: &str) -> Result<Option<RegistryNode>, InternalError>;

    /// Lists the nodes in the registry.
    ///
    /// # Arguments
    ///
    /// * `filter` - Filter to apply to the list of nodes
    fn list_nodes(&self, filter: Option<&str>) -> Result<RegistryNodeListSlice, InternalError>;

    /// Update a node in the registry.
    ///
    /// # Arguments
    ///
    /// * `node` - The updated node replacing the current node in the registry
    fn update_node(&self, node: &RegistryNode) -> Result<(), InternalError>;

    /// Delete a node from the registry.
    ///
    /// # Arguments
    ///
    /// * `identity` - The id of the node to be deleted
    fn delete_node(&self, identity: &str) -> Result<(), InternalError>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegistryNode {
    pub identity: String,
    pub endpoints: Vec<String>,
    pub display_name: String,
    pub keys: Vec<String>,
    pub metadata: HashMap<String, String>,
}

impl fmt::Display for RegistryNode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut display_string = format!("identity: {}\nendpoints:", self.identity);
        for endpoint in &self.endpoints {
            write!(display_string, "\n  - {}", endpoint)?;
        }
        write!(
            display_string,
            "\ndisplay name: {}\nkeys:",
            self.display_name
        )?;
        for key in &self.keys {
            write!(display_string, "\n  - {}", key)?;
        }
        write!(display_string, "\nmetadata:")?;
        for (key, value) in &self.metadata {
            write!(display_string, "\n  - {}: {}", key, value)?;
        }
        write!(f, "{}", display_string)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegistryNodeListSlice {
    pub data: Vec<RegistryNode>,
    pub paging: Paging,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Paging {
    pub current: String,
    pub offset: usize,
    pub limit: usize,
    pub total: usize,
    pub first: String,
    pub prev: String,
    pub next: String,
    pub last: String,
}
