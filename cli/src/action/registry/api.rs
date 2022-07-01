// Copyright 2020 Cargill Incorporated
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

use std::fmt;
use std::fmt::Write as _;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::action::api::{ServerError, SplinterRestClient};
use crate::error::CliError;

impl SplinterRestClient {
    /// Adds a new node to the registry.
    pub fn add_node(&self, node: &RegistryNode) -> Result<(), CliError> {
        let request = Client::new()
            .post(&format!("{}/registry/nodes", self.url))
            .json(&node)
            .header("Authorization", &self.auth);

        request
            .send()
            .map_err(|err| CliError::ActionError(format!("Failed to add node to registry: {}", err)))
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    Ok(())
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|_| {
                            CliError::ActionError(format!(
                                "Registry add node request failed with status code '{}', but error response was not valid",
                                status
                            ))
                        })?
                        .message;

                    Err(CliError::ActionError(format!(
                        "Failed to add node to registry: {}",
                        message
                    )))
                }
            })
    }

    /// Retrieves the node with the given identity from the registry.
    pub fn get_node(&self, identity: &str) -> Result<Option<RegistryNode>, CliError> {
        let request = Client::new()
            .get(&format!("{}/registry/nodes/{}", self.url, &identity))
            .header("Authorization", &self.auth);

        request.send()
            .map_err(|err| CliError::ActionError(format!("Failed to fetch node: {}", err)))
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    res.json::<RegistryNode>().map(Some).map_err(|_| {
                        CliError::ActionError(
                            "Request was successful, but received an invalid response".into(),
                        )
                    })
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|_| {
                            CliError::ActionError(format!(
                                "Registry get node request failed with status code '{}', but error response was not valid",
                                status
                            ))
                        })?
                        .message;

                    Err(CliError::ActionError(format!(
                        "Failed to fetch node: {}",
                        message
                    )))
                }
            })
    }
}

#[cfg(feature = "registry")]
#[derive(Debug, Deserialize, Serialize)]
pub struct RegistryNode {
    pub identity: String,
    pub endpoints: Vec<String>,
    pub display_name: String,
    pub keys: Vec<String>,
    pub metadata: HashMap<String, String>,
}

#[cfg(feature = "registry")]
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
        display_string += "\nmetadata:";
        for (key, value) in &self.metadata {
            write!(display_string, "\n  {}: {}", key, value)?;
        }
        write!(f, "{}", display_string)
    }
}
