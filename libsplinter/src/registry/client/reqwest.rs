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

//! A Reqwest-based implementation of RegistryClient

use reqwest::{blocking::Client, StatusCode};

use crate::error::InternalError;
use crate::protocol::REGISTRY_PROTOCOL_VERSION;

use super::{RegistryClient, RegistryNode, RegistryNodeListSlice};

const PAGING_LIMIT: &str = "1000";

#[derive(Deserialize)]
struct ServerError {
    pub message: String,
}

pub struct ReqwestRegistryClient {
    pub url: String,
    pub auth: String,
}

impl ReqwestRegistryClient {
    pub fn new(url: String, auth: String) -> Self {
        ReqwestRegistryClient { url, auth }
    }
}

impl RegistryClient for ReqwestRegistryClient {
    /// Add the given `node` to the registry.
    fn add_node(&self, node: &RegistryNode) -> Result<(), InternalError> {
        let request = Client::new()
            .post(&format!("{}/registry/nodes", self.url))
            .json(&node)
            .header("SplinterProtocolVersion", REGISTRY_PROTOCOL_VERSION)
            .header("Authorization", &self.auth);

        request
            .send()
            .map_err(|err| InternalError::from_source_with_message(Box::new(err), "Failed to add node to registry".to_string()))
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    Ok(())
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|err| {
                            InternalError::from_source_with_message(
                                err.into(),
                                format!(
                                    "Registry add node request failed with status code '{}', but error \
                                 response was not valid",
                                    status
                                ),
                            )
                        })?
                        .message;

                        Err(InternalError::with_message(format!(
                            "Failed to add node to registry: {}",
                            message
                        )))
                }
            })
    }

    /// Retrieve the node with the given `identity` from the registry.
    fn get_node(&self, identity: &str) -> Result<Option<RegistryNode>, InternalError> {
        let request = Client::new()
            .get(&format!("{}/registry/nodes/{}", self.url, &identity))
            .header("SplinterProtocolVersion", REGISTRY_PROTOCOL_VERSION)
            .header("Authorization", &self.auth);

        request
            .send()
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to fetch node".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    res.json::<RegistryNode>().map(Some).map_err(|_| {
                        InternalError::with_message(
                            "Request was successful, but received an invalid response".into(),
                        )
                    })
                } else if status == StatusCode::NOT_FOUND {
                    Ok(None)
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|_| {
                            InternalError::with_message(format!(
                            "Registry get node request failed with status code '{}', but error \
                             response was not valid",
                            status
                        ))
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to get node from registry: {}",
                        message
                    )))
                }
            })
    }

    /// List the nodes in the registry.
    fn list_nodes(&self, filter: Option<&str>) -> Result<RegistryNodeListSlice, InternalError> {
        let mut url = format!("{}/registry/nodes?limit={}", self.url, PAGING_LIMIT);
        if let Some(filter) = filter {
            url = format!("{}&filter={}", &url, &filter);
        }

        let request = Client::new()
            .get(&url)
            .header("SplinterProtocolVersion", REGISTRY_PROTOCOL_VERSION)
            .header("Authorization", &self.auth);

        request.send()
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to list registry nodes".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    res.json::<RegistryNodeListSlice>().map_err(|_| {
                        InternalError::with_message(
                            "Request was successful, but received an invalid response".into(),
                        )
                    })
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|err| {
                            InternalError::from_source_with_message(
                                err.into(),
                                format!(
                                    "Registry list nodes request failed with status code '{}', but error \
                                 response was not valid",
                                    status
                                ),
                            )
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to list nodes: {} {}",
                        message,
                        url
                    )))
                }
            })
    }

    /// Update the node in the registry with the same id as the given `node`.
    fn update_node(&self, node: &RegistryNode) -> Result<(), InternalError> {
        let request = Client::new()
            .put(&format!("{}/registry/nodes/{}", self.url, node.identity))
            .json(&node)
            .header("SplinterProtocolVersion", REGISTRY_PROTOCOL_VERSION)
            .header("Authorization", &self.auth);

        request
            .send()
            .map_err(|err| InternalError::from_source_with_message(Box::new(err), "Failed to replace registry node".to_string()))
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    Ok(())
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|err| {
                            InternalError::from_source_with_message(
                                err.into(),
                                format!(
                                    "Registry replace node request failed with status code '{}', but error \
                                 response was not valid",
                                    status
                                ),
                            )
                        })?
                        .message;

                        Err(InternalError::with_message(format!(
                            "Failed to replace registry node: {}",
                            message
                        )))
                }
            })
    }

    /// Delete the node with the given `identity` from the registry.
    fn delete_node(&self, identity: &str) -> Result<(), InternalError> {
        let request = Client::new()
            .delete(&format!("{}/registry/nodes/{}", self.url, identity))
            .header("SplinterProtocolVersion", REGISTRY_PROTOCOL_VERSION)
            .header("Authorization", &self.auth);

        request
            .send()
            .map_err(|err| InternalError::from_source_with_message(Box::new(err), "Failed to delete node from the registry".to_string()))
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    Ok(())
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|err| {
                            InternalError::from_source_with_message(
                                err.into(),
                                format!(
                                    "Registry delete node request failed with status code '{}', but error \
                                 response was not valid",
                                    status
                                ),
                            )
                        })?
                        .message;

                        Err(InternalError::with_message(format!(
                            "Failed to delete node from the registry: {}",
                            message
                        )))
                }
            })
    }
}
