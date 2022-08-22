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

use std::collections::BTreeMap;
use std::fmt;
use std::fmt::Write as _;

use reqwest::{blocking::Client, header, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::error::Result as JsonResult;
use splinter::admin::messages::CircuitStatus;

use crate::action::api::{ServerError, SplinterRestClient};
use crate::error::CliError;

const PAGING_LIMIT: &str = "1000";
// The admin protocol version supported by the current CLI
const CLI_ADMIN_PROTOCOL_VERSION: &str = "2";

impl SplinterRestClient {
    /// Submits an admin payload to this client's Splinter node.
    pub fn submit_admin_payload(&self, payload: Vec<u8>) -> Result<(), CliError> {
        Client::new()
            .post(&format!("{}/admin/submit", self.url))
            .header(header::CONTENT_TYPE, "octet-stream")
            .header("SplinterProtocolVersion", CLI_ADMIN_PROTOCOL_VERSION)
            .header("Authorization", &self.auth)
            .body(payload)
            .send()
            .map_err(|err| {
                CliError::ActionError(format!("Failed to submit admin payload: {}", err))
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
                                "Admin payload submit request failed with status code '{}', but \
                                 error response was not valid",
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

    pub fn list_circuits(
        &self,
        member_filter: Option<&str>,
        status_filter: Option<&str>,
    ) -> Result<CircuitListSlice, CliError> {
        let mut url = format!("{}/admin/circuits?limit={}", self.url, PAGING_LIMIT);
        if let Some(member_filter) = member_filter {
            url = format!("{}&filter={}", &url, &member_filter);
        }
        if let Some(status_filter) = status_filter {
            url = format!("{}&status={}", &url, &status_filter);
        }

        Client::new()
            .get(&url)
            .header("SplinterProtocolVersion", CLI_ADMIN_PROTOCOL_VERSION)
            .header("Authorization", &self.auth)
            .send()
            .map_err(|err| CliError::ActionError(format!("Failed to list circuits: {}", err)))
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    res.json::<CircuitListSlice>().map_err(|_| {
                        CliError::ActionError(
                            "Request was successful, but received an invalid response".into(),
                        )
                    })
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|_| {
                            CliError::ActionError(format!(
                                "Circuit list request failed with status code '{}', but error \
                                 response was not valid",
                                status
                            ))
                        })?
                        .message;

                    Err(CliError::ActionError(format!(
                        "Failed to list circuits: {}",
                        message
                    )))
                }
            })
    }

    pub fn fetch_circuit(&self, circuit_id: &str) -> Result<Option<CircuitSlice>, CliError> {
        Client::new()
            .get(&format!("{}/admin/circuits/{}", self.url, circuit_id))
            .header("SplinterProtocolVersion", CLI_ADMIN_PROTOCOL_VERSION)
            .header("Authorization", &self.auth)
            .send()
            .map_err(|err| CliError::ActionError(format!("Failed to fetch circuit: {}", err)))
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    res.json::<CircuitSlice>().map(Some).map_err(|_| {
                        CliError::ActionError(
                            "Request was successful, but received an invalid response".into(),
                        )
                    })
                } else if status == StatusCode::NOT_FOUND {
                    Ok(None)
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|_| {
                            CliError::ActionError(format!(
                                "Circuit fetch request failed with status code '{}', but error \
                                 response was not valid",
                                status
                            ))
                        })?
                        .message;

                    Err(CliError::ActionError(format!(
                        "Failed to fetch circuit: {}",
                        message
                    )))
                }
            })
    }

    pub fn list_proposals(
        &self,
        management_type_filter: Option<&str>,
        member_filter: Option<&str>,
    ) -> Result<ProposalListSlice, CliError> {
        let mut filters = vec![];
        if let Some(management_type) = management_type_filter {
            filters.push(format!("management_type={}", management_type));
        }
        if let Some(member) = member_filter {
            filters.push(format!("member={}", member));
        }

        let mut url = format!("{}/admin/proposals?limit={}", self.url, PAGING_LIMIT);
        if !filters.is_empty() {
            write!(url, "&{}", filters.join("&"))
                .map_err(|e| CliError::ActionError(e.to_string()))?;
        }

        Client::new()
            .get(&url)
            .header("SplinterProtocolVersion", CLI_ADMIN_PROTOCOL_VERSION)
            .header("Authorization", &self.auth)
            .send()
            .map_err(|err| CliError::ActionError(format!("Failed to list proposals: {}", err)))
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    res.json::<ProposalListSlice>().map_err(|_| {
                        CliError::ActionError(
                            "Request was successful, but received an invalid response".into(),
                        )
                    })
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|_| {
                            CliError::ActionError(format!(
                                "Proposal list request failed with status code '{}', but error \
                                 response was not valid",
                                status
                            ))
                        })?
                        .message;

                    Err(CliError::ActionError(format!(
                        "Failed to list proposals: {}",
                        message
                    )))
                }
            })
    }

    pub fn fetch_proposal(&self, circuit_id: &str) -> Result<Option<ProposalSlice>, CliError> {
        Client::new()
            .get(&format!("{}/admin/proposals/{}", self.url, circuit_id))
            .header("SplinterProtocolVersion", CLI_ADMIN_PROTOCOL_VERSION)
            .header("Authorization", &self.auth)
            .send()
            .map_err(|err| CliError::ActionError(format!("Failed to fetch proposal: {}", err)))
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    res.json::<ProposalSlice>().map(Some).map_err(|_| {
                        CliError::ActionError(
                            "Request was successful, but received an invalid response".into(),
                        )
                    })
                } else if status == StatusCode::NOT_FOUND {
                    Ok(None)
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|_| {
                            CliError::ActionError(format!(
                                "Proposal fetch request failed with status code '{}', but error \
                                 response was not valid",
                                status
                            ))
                        })?
                        .message;

                    Err(CliError::ActionError(format!(
                        "Failed to fetch proposal: {}",
                        message
                    )))
                }
            })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CircuitSlice {
    pub id: String,
    pub members: Vec<CircuitMembers>,
    pub roster: Vec<CircuitServiceSlice>,
    pub management_type: String,
    pub display_name: Option<String>,
    pub circuit_version: i32,
    pub circuit_status: Option<CircuitStatus>,
}

impl fmt::Display for CircuitSlice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut display_string = format!("Circuit: {}\n    ", self.id,);

        if let Some(display_name) = &self.display_name {
            writeln!(display_string, "Display Name: {}    ", display_name)?;
        } else {
            writeln!(display_string, "Display Name: -    ")?;
        }

        if let Some(status) = &self.circuit_status {
            writeln!(display_string, "    Circuit Status: {}    ", status)?;
        } else {
            display_string += "    Circuit Status: Active\n    ";
        }

        writeln!(
            display_string,
            "    Schema Version: {}\n    Management Type: {}",
            self.circuit_version, self.management_type
        )?;

        for member in self.members.iter() {
            writeln!(display_string, "\n    {}", member.node_id)?;
            if let Some(public_key) = &member.public_key {
                writeln!(display_string, "        Public Key: {}", public_key)?;
            }

            display_string += "        Endpoints:\n";
            for endpoint in member.endpoints.iter() {
                writeln!(display_string, "            {}", endpoint)?;
            }

            for service in self.roster.iter() {
                if member.node_id == service.node_id {
                    writeln!(
                        display_string,
                        "        Service ({}): {}",
                        service.service_type, service.service_id
                    )?;

                    for (key, value) in &service.arguments {
                        writeln!(display_string, "          {}:", key)?;
                        // break apart value if its a list
                        if value.starts_with('[') && value.ends_with(']') {
                            let values: JsonResult<Vec<String>> = serde_json::from_str(value);
                            match values {
                                Ok(values) => {
                                    for i in values {
                                        writeln!(display_string, "              {}", i)?;
                                    }
                                }
                                Err(_) => writeln!(display_string, "              {}", value)?,
                            };
                        } else {
                            let values =
                                value.split(',').map(String::from).collect::<Vec<String>>();
                            for value in values {
                                writeln!(display_string, "              {}", value)?;
                            }
                        }
                    }
                }
            }
        }

        write!(f, "{}", display_string)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CircuitServiceSlice {
    pub service_id: String,
    pub service_type: String,
    pub node_id: String,
    pub arguments: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CircuitListSlice {
    pub data: Vec<CircuitSlice>,
    pub paging: Paging,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ProposalSlice {
    pub proposal_type: String,
    pub circuit_id: String,
    pub circuit_hash: String,
    pub circuit: ProposalCircuitSlice,
    pub votes: Vec<VoteRecord>,
    pub requester: String,
    pub requester_node_id: String,
}

impl fmt::Display for ProposalSlice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut display_string = if self.proposal_type == "Disband" {
            format!("Proposal to disband: {}\n    ", self.circuit_id)
        } else {
            format!("Proposal to create: {}\n    ", self.circuit_id)
        };

        if let Some(display_name) = &self.circuit.display_name {
            writeln!(display_string, "Display Name: {}    ", display_name)?;
        } else {
            display_string += "Display Name: -\n    ";
        }

        if let Some(status) = &self.circuit.circuit_status {
            writeln!(display_string, "Circuit Status: {}    ", status)?;
        } else {
            display_string += "Circuit Status: Active\n    ";
        }

        write!(
            display_string,
            "    Schema Version: {}\n    Management Type: {}\n",
            self.circuit.circuit_version, self.circuit.management_type
        )?;

        for member in self.circuit.members.iter() {
            write!(display_string, "\n    {}\n", member.node_id)?;
            if let Some(public_key) = &member.public_key {
                writeln!(display_string, "        Public Key: {}", public_key)?;
            }
            if member.node_id == self.requester_node_id {
                display_string += "        Vote: ACCEPT (implied as requester):\n";
                writeln!(display_string, "            {}", self.requester)?;
            } else {
                let mut vote_string = "        Vote: PENDING".to_string();
                for vote in self.votes.iter() {
                    if vote.voter_node_id == member.node_id {
                        vote_string =
                            format!("        Vote: ACCEPT\n             {}", vote.public_key)
                    }
                }
                writeln!(display_string, "{}", vote_string)?;
            }
            display_string += "        Endpoints:\n";
            for endpoint in member.endpoints.iter() {
                writeln!(display_string, "            {}", endpoint)?;
            }

            for service in self.circuit.roster.iter() {
                if service.node_id == member.node_id {
                    writeln!(
                        display_string,
                        "        Service ({}): {}",
                        service.service_type, service.service_id
                    )?;

                    for key_value in service.arguments.iter() {
                        let key = &key_value[0];
                        let value = &key_value[1];
                        writeln!(display_string, "            {}:", key)?;
                        if value.starts_with('[') && value.ends_with(']') {
                            let values: JsonResult<Vec<String>> = serde_json::from_str(value);
                            match values {
                                Ok(values) => {
                                    for i in values {
                                        writeln!(display_string, "                {}", i)?;
                                    }
                                }
                                Err(_) => writeln!(display_string, "                {}", value)?,
                            };
                        } else {
                            let values =
                                value.split(',').map(String::from).collect::<Vec<String>>();
                            for value in values {
                                writeln!(display_string, "              {}", value)?;
                            }
                        }
                    }
                }
            }
        }

        write!(f, "{}", display_string)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ProposalCircuitSlice {
    pub circuit_id: String,
    pub members: Vec<CircuitMembers>,
    pub roster: Vec<CircuitService>,
    pub management_type: String,
    pub comments: Option<String>,
    pub display_name: Option<String>,
    pub circuit_version: i32,
    pub circuit_status: Option<CircuitStatus>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CircuitMembers {
    pub node_id: String,
    pub endpoints: Vec<String>,
    pub public_key: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct CircuitService {
    pub service_id: String,
    pub service_type: String,
    pub node_id: String,
    pub arguments: Vec<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ProposalListSlice {
    pub data: Vec<ProposalSlice>,
    pub paging: Paging,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct VoteRecord {
    pub public_key: String,
    pub vote: String,
    pub voter_node_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
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
