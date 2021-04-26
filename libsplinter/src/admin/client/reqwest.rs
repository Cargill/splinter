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

//! Contains the Reqwest-based implementation of AdminServiceClient.

use reqwest::{blocking::Client, header, StatusCode};

use crate::error::InternalError;
use crate::protocol::ADMIN_PROTOCOL_VERSION;

use super::{AdminServiceClient, CircuitListSlice, CircuitSlice, ProposalListSlice, ProposalSlice};

const PAGING_LIMIT: u32 = 100;

#[derive(Deserialize)]
struct ServerError {
    pub message: String,
}

pub struct ReqwestAdminServiceClient {
    url: String,
    auth: String,
}

impl ReqwestAdminServiceClient {
    pub fn new(url: String, auth: String) -> Self {
        ReqwestAdminServiceClient { url, auth }
    }
}

impl AdminServiceClient for ReqwestAdminServiceClient {
    /// Submits an admin payload to this client's Splinter node.
    fn submit_admin_payload(&self, payload: Vec<u8>) -> Result<(), InternalError> {
        let request = Client::new()
            .post(&format!("{}/admin/submit", self.url))
            .header(header::CONTENT_TYPE, "octet-stream")
            .header("SplinterProtocolVersion", ADMIN_PROTOCOL_VERSION)
            .body(payload)
            .header("Authorization", &self.auth);

        request
            .send()
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to submit admin payload".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    Ok(())
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|_| {
                            InternalError::with_message(format!(
                                "Admin payload submit request failed with status code '{}', but \
                                 error response was not valid",
                                status
                            ))
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to submit admin payload: {}",
                        message
                    )))
                }
            })
    }

    fn list_circuits(&self, filter: Option<&str>) -> Result<CircuitListSlice, InternalError> {
        let mut url = format!("{}/admin/circuits?limit={}", self.url, PAGING_LIMIT);
        if let Some(filter) = filter {
            if filter.starts_with("status") {
                url = format!("{}&{}", &url, &filter);
            } else {
                url = format!("{}&filter={}", &url, &filter);
            }
        }

        let request = Client::new()
            .get(&url)
            .header("SplinterProtocolVersion", ADMIN_PROTOCOL_VERSION)
            .header("Authorization", &self.auth);

        let response = request.send();

        response
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to list circuits".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    res.json::<CircuitListSlice>().map_err(|_| {
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
                                    "Circuit list request failed with status code '{}', but error \
                                 response was not valid",
                                    status
                                ),
                            )
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to list circuits: {}",
                        message
                    )))
                }
            })
    }

    fn fetch_circuit(&self, circuit_id: &str) -> Result<Option<CircuitSlice>, InternalError> {
        let request = Client::new()
            .get(&format!("{}/admin/circuits/{}", self.url, circuit_id))
            .header("SplinterProtocolVersion", ADMIN_PROTOCOL_VERSION)
            .header("Authorization", &self.auth);

        request
            .send()
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to fetch circuit".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    res.json::<CircuitSlice>().map(Some).map_err(|_| {
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
                                "Circuit fetch request failed with status code '{}', but error \
                                 response was not valid",
                                status
                            ))
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to fetch circuit: {}",
                        message
                    )))
                }
            })
    }

    fn list_proposals(
        &self,
        management_type_filter: Option<&str>,
        member_filter: Option<&str>,
    ) -> Result<ProposalListSlice, InternalError> {
        let mut filters = vec![];
        if let Some(management_type) = management_type_filter {
            filters.push(format!("management_type={}", management_type));
        }
        if let Some(member) = member_filter {
            filters.push(format!("member={}", member));
        }

        let mut url = format!("{}/admin/proposals?limit={}", self.url, PAGING_LIMIT);
        if !filters.is_empty() {
            url.push_str(&format!("&{}", filters.join("&")));
        }

        let request = Client::new()
            .get(&url)
            .header("SplinterProtocolVersion", ADMIN_PROTOCOL_VERSION)
            .header("Authorization", &self.auth);

        request
            .send()
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to list proposals".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    res.json::<ProposalListSlice>().map_err(|_| {
                        InternalError::with_message(
                            "Request was successful, but received an invalid response".into(),
                        )
                    })
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|_| {
                            InternalError::with_message(format!(
                                "Proposal list request failed with status code '{}', but error \
                                 response was not valid",
                                status
                            ))
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to list proposals: {}",
                        message
                    )))
                }
            })
    }

    fn fetch_proposal(&self, circuit_id: &str) -> Result<Option<ProposalSlice>, InternalError> {
        let request = Client::new()
            .get(&format!("{}/admin/proposals/{}", self.url, circuit_id))
            .header("SplinterProtocolVersion", ADMIN_PROTOCOL_VERSION)
            .header("Authorization", &self.auth);

        request
            .send()
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to fetch proposal".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    res.json::<ProposalSlice>().map(Some).map_err(|_| {
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
                                "Proposal fetch request failed with status code '{}', but error \
                                 response was not valid",
                                status
                            ))
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to fetch proposal: {}",
                        message
                    )))
                }
            })
    }
}
