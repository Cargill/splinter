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

use std::collections::BTreeMap;

use serde::Serialize;

use crate::admin::store::{AdminServiceStore, Circuit, CircuitPredicate, CircuitStatus, Service};
use crate::rest_api::paging::{get_response_paging_info, Paging};
use crate::rest_error::RESTError;

pub struct Arguments {
    pub store: Box<dyn AdminServiceStore>,
    pub offset: usize,
    pub limit: usize,
    pub link: String,
    pub status: Option<String>,
    pub member: Option<String>,
}

pub fn get_admin_circuits(args: Arguments) -> Result<Response, RESTError> {
    let mut filters = {
        if let Some(member) = args.member {
            vec![CircuitPredicate::MembersInclude(vec![format!(
                "filter={}",
                member
            )])]
        } else {
            vec![]
        }
    };
    if let Some(status) = args.status {
        filters.push(CircuitPredicate::CircuitStatus(CircuitStatus::from(
            format!("status={}", status),
        )));
    }
    let circuits = args
        .store
        .list_circuits(&filters)
        .map_err(|e| RESTError::internal_error("Error getting circuits", Some(Box::new(e))))?;
    let offset_value = args.offset;
    let total = circuits.len();
    let limit_value = args.limit;

    let data = circuits
        .skip(offset_value)
        .take(limit_value)
        .map(CircuitResponse::from)
        .collect::<Vec<_>>();

    let paging = get_response_paging_info(Some(args.limit), Some(args.offset), &args.link, total);
    Ok(Response { data, paging })
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct Response {
    pub data: Vec<CircuitResponse>,
    pub paging: Paging,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct CircuitResponse {
    pub id: Box<str>,
    pub members: Vec<String>,
    pub roster: Vec<ServiceResponse>,
    pub management_type: Box<str>,
}

impl From<Circuit> for CircuitResponse {
    fn from(circuit: Circuit) -> Self {
        Self {
            id: circuit.circuit_id().to_string().into(),
            members: circuit
                .members()
                .iter()
                .map(|node| node.node_id().to_string())
                .collect(),
            roster: circuit.roster().iter().map(ServiceResponse::from).collect(),
            management_type: circuit.circuit_management_type().to_string().into(),
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct ServiceResponse {
    pub service_id: Box<str>,
    pub service_type: Box<str>,
    pub allowed_nodes: Vec<String>,
    pub arguments: BTreeMap<String, String>,
}

impl From<&Service> for ServiceResponse {
    fn from(service_def: &Service) -> Self {
        Self {
            service_id: service_def.service_id().to_string().into(),
            service_type: service_def.service_type().to_string().into(),
            allowed_nodes: vec![service_def.node_id().to_string()],
            arguments: service_def
                .arguments()
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect::<BTreeMap<String, String>>(),
        }
    }
}
