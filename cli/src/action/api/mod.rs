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

#[cfg(feature = "authorization-handler-rbac")]
mod rbac;

use reqwest::blocking::Client;
use serde::Deserialize;

use super::CliError;

#[cfg(feature = "authorization-handler-rbac")]
pub use rbac::{Role, RoleBuilder, RoleUpdate, RoleUpdateBuilder};

#[derive(Default)]
pub struct SplinterRestClientBuilder {
    pub url: Option<String>,
    pub auth: Option<String>,
}

impl SplinterRestClientBuilder {
    pub fn new() -> Self {
        SplinterRestClientBuilder::default()
    }

    pub fn with_url(mut self, url: String) -> Self {
        self.url = Some(url);
        self
    }

    pub fn with_auth(mut self, auth: String) -> Self {
        self.auth = Some(auth);
        self
    }

    pub fn build(self) -> Result<SplinterRestClient, CliError> {
        Ok(SplinterRestClient {
            url: self.url.ok_or_else(|| {
                CliError::ActionError("Failed to build client, url not provided".to_string())
            })?,
            auth: self.auth.ok_or_else(|| {
                CliError::ActionError(
                    "Failed to build client, jwt authorization not provided".to_string(),
                )
            })?,
        })
    }
}

/// A wrapper around the Splinter REST API.
pub struct SplinterRestClient {
    pub url: String,
    pub auth: String,
}

impl SplinterRestClient {
    /// Gets the Splinter node's status.
    pub fn get_node_status(&self) -> Result<NodeStatus, CliError> {
        Client::new()
            .get(&format!("{}/status", self.url))
            .header("Authorization", &self.auth)
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

    /// Checks whether or not maintenance mode is enabled for the Splinter node.
    #[cfg(feature = "authorization-handler-maintenance")]
    pub fn is_maintenance_mode_enabled(&self) -> Result<bool, CliError> {
        Client::new()
            .get(&format!("{}/authorization/maintenance", self.url))
            .header("Authorization", &self.auth)
            .send()
            .map_err(|err| {
                CliError::ActionError(format!("Failed to check maintenance mode status: {}", err))
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    res.text()
                        .map_err(|err| {
                            CliError::ActionError(format!(
                                "Request was successful, but failed to parse response body: {}",
                                err
                            ))
                        })?
                        .parse()
                        .map_err(|_| {
                            CliError::ActionError(
                                "Request was successful, but received an invalid response".into(),
                            )
                        })
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|_| {
                            CliError::ActionError(format!(
                                "Maintenance mode check request failed with status code '{}', but \
                                 error response was not valid",
                                status
                            ))
                        })?
                        .message;

                    Err(CliError::ActionError(format!(
                        "Failed to check maintenance mode status: {}",
                        message
                    )))
                }
            })
    }

    /// Turns maintenance mode on or off for the Splinter node.
    #[cfg(feature = "authorization-handler-maintenance")]
    pub fn set_maintenance_mode(&self, enabled: bool) -> Result<(), CliError> {
        Client::new()
            .post(&format!("{}/authorization/maintenance", self.url))
            .query(&[("enabled", enabled)])
            .header("Authorization", &self.auth)
            .send()
            .map_err(|err| {
                CliError::ActionError(format!("Failed to set maintenance mode: {}", err))
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    Ok(())
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|_| {
                            CliError::ActionError(format!(
                                "Maintenance mode set request failed with status code '{}', but \
                                 error response was not valid",
                                status
                            ))
                        })?
                        .message;

                    Err(CliError::ActionError(format!(
                        "Failed to set maintenance mode: {}",
                        message
                    )))
                }
            })
    }

    /// Lists all REST API permissions for a Splinter node.
    #[cfg(feature = "permissions")]
    pub fn list_permissions(&self) -> Result<Vec<Permission>, CliError> {
        Client::new()
            .get(&format!("{}/authorization/permissions", self.url))
            .header("Authorization", &self.auth)
            .send()
            .map_err(|err| CliError::ActionError(format!("Failed to get permissions: {}", err)))
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    res.json::<PermissionsResponse>()
                        .map(|mut response| {
                            response.data.sort_by(|a, b| {
                                // Unwrapping because comparing strings always returns `Some(_)`
                                a.permission_id.partial_cmp(&b.permission_id).unwrap()
                            });
                            response.data
                        })
                        .map_err(|_| {
                            CliError::ActionError(
                                "Request was successful, but received an invalid response".into(),
                            )
                        })
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|_| {
                            CliError::ActionError(format!(
                                "Permissions list request failed with status code '{}', but \
                                 error response was not valid",
                                status
                            ))
                        })?
                        .message;

                    Err(CliError::ActionError(format!(
                        "Failed to get permissions list: {}",
                        message
                    )))
                }
            })
    }

    #[cfg(feature = "authorization-handler-rbac")]
    pub fn list_roles(&self) -> Result<rbac::RoleIter, CliError> {
        Ok(rbac::RoleIter::new(&self.url, &self.auth))
    }

    #[cfg(feature = "authorization-handler-rbac")]
    pub fn get_role(&self, role_id: &str) -> Result<Role, CliError> {
        rbac::get_role(&self.url, &self.auth, role_id)
    }

    #[cfg(feature = "authorization-handler-rbac")]
    pub fn create_role(&self, role: Role) -> Result<(), CliError> {
        rbac::create_role(&self.url, &self.auth, role)
    }

    #[cfg(feature = "authorization-handler-rbac")]
    pub fn update_role(&self, role_update: RoleUpdate) -> Result<(), CliError> {
        rbac::update_role(&self.url, &self.auth, role_update)
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
    pub network_endpoints: Vec<String>,
    pub advertised_endpoints: Vec<String>,
    pub version: String,
}

#[derive(Deserialize)]
struct PermissionsResponse {
    pub data: Vec<Permission>,
}

#[derive(Deserialize)]
pub struct Permission {
    pub permission_id: String,
    pub permission_display_name: String,
    pub permission_description: String,
}
