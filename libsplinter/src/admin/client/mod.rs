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

//! Traits and implementations useful for communicating with the Splinter admin service as
//! a client.

#[cfg(feature = "client-reqwest")]
mod reqwest;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::InternalError;

pub use self::reqwest::ReqwestAdminServiceClient;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CircuitServiceSlice {
    pub service_id: String,
    pub service_type: String,
    pub node_id: String,
    pub arguments: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CircuitSlice {
    pub id: String,
    pub members: Vec<String>,
    pub roster: Vec<CircuitServiceSlice>,
    pub management_type: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CircuitListSlice {
    pub data: Vec<CircuitSlice>,
    pub paging: Paging,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CircuitMembers {
    pub node_id: String,
    pub endpoints: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CircuitService {
    pub service_id: String,
    pub service_type: String,
    pub node_id: String,
    pub arguments: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProposalCircuitSlice {
    pub circuit_id: String,
    pub members: Vec<CircuitMembers>,
    pub roster: Vec<CircuitService>,
    pub management_type: String,
    pub comments: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoteRecord {
    pub public_key: String,
    pub vote: String,
    pub voter_node_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProposalSlice {
    pub proposal_type: String,
    pub circuit_id: String,
    pub circuit_hash: String,
    pub circuit: ProposalCircuitSlice,
    pub votes: Vec<VoteRecord>,
    pub requester: String,
    pub requester_node_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProposalListSlice {
    pub data: Vec<ProposalSlice>,
    pub paging: Paging,
}

pub trait AdminServiceClient {
    fn submit_admin_payload(&self, payload: Vec<u8>) -> Result<(), InternalError>;
    fn list_circuits(&self, filter: Option<&str>) -> Result<CircuitListSlice, InternalError>;
    fn fetch_circuit(&self, circuit_id: &str) -> Result<Option<CircuitSlice>, InternalError>;
    fn list_proposals(
        &self,
        management_type_filter: Option<&str>,
        member_filter: Option<&str>,
    ) -> Result<ProposalListSlice, InternalError>;
    fn fetch_proposal(&self, circuit_id: &str) -> Result<Option<ProposalSlice>, InternalError>;
}
