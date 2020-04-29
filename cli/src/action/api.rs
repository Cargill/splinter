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

//! Provides convenient functions for sending REST API requests to a splinter node.

use reqwest::blocking::Client;
use serde::Deserialize;

use super::CliError;

/// A wrapper around the Splinter REST API.
pub struct SplinterRestClient<'a> {
    pub url: &'a str,
}

impl<'a> SplinterRestClient<'a> {
    /// Constructs a new client for a Splinter node at the given URL.
    pub fn new(url: &'a str) -> Self {
        Self { url }
    }

    /// Gets the Splinter node's status.
    pub fn get_node_status(&self) -> Result<NodeStatus, CliError> {
        Client::new()
            .get(&format!("{}/status", self.url))
            .send()
            .map_err(|err| CliError::ActionError(format!("Failed to fetch node ID: {}", err)))
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    res.json::<NodeStatus>().map_err(|_| {
                        CliError::ActionError(
                            "Request was successful, but received an invalid response".into(),
                        )
                    })
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|_| {
                            CliError::ActionError(format!(
                                "Node ID fetch request failed with status code '{}', but error \
                                 response was not valid",
                                status
                            ))
                        })?
                        .message;

                    Err(CliError::ActionError(format!(
                        "Failed to submit admin payload: {}",
                        message
                    )))
                }
            })
    }
}

#[derive(Deserialize)]
pub struct ServerError {
    pub message: String,
}

#[derive(Deserialize)]
pub struct NodeStatus {
    pub node_id: String,
    pub display_name: String,
    pub service_endpoint: String,
    pub network_endpoints: Vec<String>,
    pub advertised_endpoints: Vec<String>,
    pub version: String,
}
