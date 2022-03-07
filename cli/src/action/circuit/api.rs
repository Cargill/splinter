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

use reqwest::{blocking::Client, header, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::error::Result as JsonResult;
use splinter::protocol::ADMIN_PROTOCOL_VERSION;

use crate::action::api::{ServerError, SplinterRestClient};
use crate::error::CliError;

const PAGING_LIMIT: &str = "1000";

#[cfg(not(feature = "circuit-display-endpoints"))]
type ReturnCircuitSlice = CircuitSlice;
#[cfg(feature = "circuit-display-endpoints")]
type ReturnCircuitSlice = CircuitSliceWithEndpoints;

impl<'a> SplinterRestClient<'a> {
    /// Submits an admin payload to this client's Splinter node.
    pub fn submit_admin_payload(&self, payload: Vec<u8>) -> Result<(), CliError> {
        Client::new()
            .post(&format!("{}/admin/submit", self.url))
            .header(header::CONTENT_TYPE, "octet-stream")
            .header("SplinterProtocolVersion", ADMIN_PROTOCOL_VERSION)
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

    pub fn list_circuits(&self, filter: Option<&str>) -> Result<CircuitListSlice, CliError> {
        let mut request = format!("{}/admin/circuits?limit={}", self.url, PAGING_LIMIT);
        if let Some(filter) = filter {
            request = format!("{}&filter={}", &request, &filter);
        }

        Client::new()
            .get(&request)
            .header("SplinterProtocolVersion", ADMIN_PROTOCOL_VERSION)
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

    pub fn fetch_circuit(&self, circuit_id: &str) -> Result<Option<ReturnCircuitSlice>, CliError> {
        let res = Client::new()
            .get(&format!("{}/admin/circuits/{}", self.url, circuit_id))
            .header("SplinterProtocolVersion", ADMIN_PROTOCOL_VERSION)
            .send()
            .map_err(|err| CliError::ActionError(format!("Failed to fetch circuit: {}", err)))?;

        let status = res.status();
        if status.is_success() {
            let circuit: Circuits = res.json::<Circuits>().map_err(|_| {
                CliError::ActionError(
                    "Request was successful, but received an invalid response".into(),
                )
            })?;

            #[cfg(not(feature = "circuit-display-endpoints"))]
            {
                return Ok(Some(CircuitSlice::from(circuit)));
            }

            #[cfg(feature = "circuit-display-endpoints")]
            {
                return Ok(Some(CircuitSliceWithEndpoints::from(circuit)));
            }
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

        let mut request = format!("{}/admin/proposals?limit={}", self.url, PAGING_LIMIT);
        if !filters.is_empty() {
            request.push_str(&format!("&{}", filters.join("&")));
        }

        Client::new()
            .get(&request)
            .header("SplinterProtocolVersion", ADMIN_PROTOCOL_VERSION)
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
            .header("SplinterProtocolVersion", ADMIN_PROTOCOL_VERSION)
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct CircuitSlice {
    pub id: String,
    pub members: Vec<String>,
    pub roster: Vec<CircuitServiceSlice>,
    pub management_type: String,
}

impl fmt::Display for CircuitSlice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut display_string = format!(
            "Circuit: {}\n    Management Type: {}\n",
            self.id, self.management_type
        );

        for member in self.members.iter() {
            display_string += &format!("\n    {}\n", member);
            for service in self.roster.iter() {
                if service.allowed_nodes.contains(member) {
                    display_string += &format!(
                        "        Service ({}): {}\n",
                        service.service_type, service.service_id
                    );

                    for (key, value) in &service.arguments {
                        display_string += &format!("          {}:\n", key);
                        // break apart value if its a list
                        if value.starts_with('[') && value.ends_with(']') {
                            let values: JsonResult<Vec<String>> = serde_json::from_str(value);
                            match values {
                                Ok(values) => {
                                    for i in values {
                                        display_string += &format!("              {}\n", i);
                                    }
                                }
                                Err(_) => display_string += &format!("              {}\n", value),
                            };
                        } else {
                            display_string += &format!("              {}\n", value);
                        }
                    }
                }
            }
        }

        write!(f, "{}", display_string)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Circuits {
    CircuitSlice {
        id: String,
        members: Vec<String>,
        roster: Vec<CircuitServiceSlice>,
        management_type: String,
    },
    CircuitSliceWithEndpoints {
        id: String,
        members: Vec<CircuitNodes>,
        roster: Vec<CircuitServiceSlice>,
        management_type: String,
    },
}

impl From<Circuits> for CircuitSlice {
    fn from(circuit: Circuits) -> Self {
        CircuitSlice::from(&circuit)
    }
}

impl<'a> From<&'a Circuits> for CircuitSlice {
    fn from(circuit: &'a Circuits) -> Self {
        match circuit {
            Circuits::CircuitSlice {
                id,
                members,
                roster,
                management_type,
            } => CircuitSlice {
                id: id.to_string(),
                members: members.to_vec(),
                roster: roster.to_vec(),
                management_type: management_type.to_string(),
            },
            Circuits::CircuitSliceWithEndpoints {
                id,
                members,
                roster,
                management_type,
            } => CircuitSlice {
                id: id.to_string(),
                members: members
                    .into_iter()
                    .map(|m| m.id.clone().to_string())
                    .collect(),
                roster: roster.to_vec(),
                management_type: management_type.to_string(),
            },
        }
    }
}

#[cfg(feature = "circuit-display-endpoints")]
impl From<Circuits> for CircuitSliceWithEndpoints {
    fn from(circuit: Circuits) -> Self {
        CircuitSliceWithEndpoints::from(&circuit)
    }
}

#[cfg(feature = "circuit-display-endpoints")]
impl<'a> From<&'a Circuits> for CircuitSliceWithEndpoints {
    fn from(circuit: &'a Circuits) -> Self {
        match circuit {
            Circuits::CircuitSlice {
                id,
                members,
                roster,
                management_type,
            } => CircuitSliceWithEndpoints {
                id: id.to_string(),
                members: members
                    .into_iter()
                    .map(|m| CircuitNodes {
                        id: m.to_string(),
                        endpoints: vec![],
                    })
                    .collect(),
                roster: roster.to_vec(),
                management_type: management_type.to_string(),
            },
            Circuits::CircuitSliceWithEndpoints {
                id,
                members,
                roster,
                management_type,
            } => CircuitSliceWithEndpoints {
                id: id.to_string(),
                members: members.to_vec(),
                roster: roster.to_vec(),
                management_type: management_type.to_string(),
            },
        }
    }
}

impl fmt::Display for Circuits {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[cfg(not(feature = "circuit-display-endpoints"))]
        {
            write!(f, "{}", CircuitSlice::from(self))
        }
        #[cfg(feature = "circuit-display-endpoints")]
        {
            write!(f, "{}", CircuitSliceWithEndpoints::from(self))
        }
    }
}

#[cfg(feature = "circuit-display-endpoints")]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct CircuitSliceWithEndpoints {
    pub id: String,
    pub members: Vec<CircuitNodes>,
    pub roster: Vec<CircuitServiceSlice>,
    pub management_type: String,
}

#[cfg(feature = "circuit-display-endpoints")]
impl fmt::Display for CircuitSliceWithEndpoints {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut display_string = format!(
            "Circuit: {}\n    Management Type: {}\n",
            self.id, self.management_type
        );

        for member in self.members.iter() {
            display_string += &format!("\n    {}\n", member.id);
            if !member.endpoints.is_empty() {
                display_string += "        Endpoints:\n";
                for endpoint in member.endpoints.iter() {
                    display_string += &format!("            {}\n", endpoint);
                }
            }
            for service in self.roster.iter() {
                if service.allowed_nodes.contains(&member.id) {
                    display_string += &format!(
                        "        Service ({}): {}\n",
                        service.service_type, service.service_id
                    );

                    for (key, value) in &service.arguments {
                        display_string += &format!("          {}:\n", key);
                        // break apart value if its a list
                        if value.starts_with('[') && value.ends_with(']') {
                            let values: JsonResult<Vec<String>> = serde_json::from_str(value);
                            match values {
                                Ok(values) => {
                                    for i in values {
                                        display_string += &format!("              {}\n", i);
                                    }
                                }
                                Err(_) => display_string += &format!("              {}\n", value),
                            };
                        } else {
                            display_string += &format!("              {}\n", value);
                        }
                    }
                }
            }
        }

        write!(f, "{}", display_string)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct CircuitNodes {
    pub id: String,
    pub endpoints: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct CircuitServiceSlice {
    pub service_id: String,
    pub service_type: String,
    pub allowed_nodes: Vec<String>,
    pub arguments: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct CircuitListSlice {
    pub data: Vec<CircuitSlice>,
    pub paging: Paging,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
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
        let mut display_string = format!(
            "Proposal to create: {}\n    Management Type: {}\n",
            self.circuit_id, self.circuit.management_type
        );

        for member in self.circuit.members.iter() {
            display_string += &format!("\n    {} ({:?})\n", member.node_id, member.endpoints);
            if member.node_id == self.requester_node_id {
                display_string += &"        Vote: ACCEPT (implied as requester):\n".to_string();
                display_string += &format!("            {}\n", self.requester);
            } else {
                let mut vote_string = "        Vote: PENDING".to_string();
                for vote in self.votes.iter() {
                    if vote.voter_node_id == member.node_id {
                        vote_string =
                            format!("        Vote: ACCEPT\n             {}", vote.public_key)
                    }
                }
                display_string += &format!("{}\n", vote_string);
            }
            for service in self.circuit.roster.iter() {
                if service.allowed_nodes.contains(&member.node_id) {
                    display_string += &format!(
                        "        Service ({}): {}\n",
                        service.service_type, service.service_id
                    );

                    for key_value in service.arguments.iter() {
                        let key = &key_value[0];
                        let value = &key_value[1];
                        display_string += &format!("            {}:\n", key);
                        if value.starts_with('[') && value.ends_with(']') {
                            let values: JsonResult<Vec<String>> = serde_json::from_str(value);
                            match values {
                                Ok(values) => {
                                    for i in values {
                                        display_string += &format!("                {}\n", i);
                                    }
                                }
                                Err(_) => display_string += &format!("                {}\n", value),
                            };
                        } else {
                            display_string += &format!("                {}\n", value);
                        }
                    }
                }
            }
        }

        write!(f, "{}", display_string)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ProposalCircuitSlice {
    pub circuit_id: String,
    pub members: Vec<CircuitMembers>,
    pub roster: Vec<CircuitService>,
    pub management_type: String,
    pub comments: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct CircuitMembers {
    pub node_id: String,
    pub endpoints: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct CircuitService {
    pub service_id: String,
    pub service_type: String,
    pub allowed_nodes: Vec<String>,
    pub arguments: Vec<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
