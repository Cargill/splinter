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

use std::convert::TryFrom;

use splinter::{
    error::{InternalError, InvalidArgumentError},
    service::{ArgumentsConverter, ServiceId},
};

use crate::store::service::ConsensusType;

use super::{ScabbardArguments, ScabbardArgumentsBuilder};

pub struct ScabbardArgumentsVecConverter {}

impl ArgumentsConverter<ScabbardArguments, Vec<(String, String)>>
    for ScabbardArgumentsVecConverter
{
    fn to_right(&self, left: ScabbardArguments) -> Result<Vec<(String, String)>, InternalError> {
        Ok(vec![(
            "peer_services".to_string(),
            left.peers()
                .iter()
                .map(|service_id| service_id.to_string())
                .collect::<Vec<String>>()
                .join(","),
        )])
    }

    fn to_left(&self, right: Vec<(String, String)>) -> Result<ScabbardArguments, InternalError> {
        let mut arg_builder = ScabbardArgumentsBuilder::new();

        for (key, value) in right {
            match key.as_str() {
                "peer_services" => {
                    let peers: Vec<ServiceId> = parse_list(&value)
                        .map_err(InternalError::with_message)?
                        .iter()
                        .map(ServiceId::new)
                        .collect::<Result<Vec<ServiceId>, InvalidArgumentError>>()
                        .map_err(|err| InternalError::from_source(Box::new(err)))?;
                    arg_builder = arg_builder.with_peers(peers);
                }
                "consensus" => {
                    let consensus = ConsensusType::try_from(value)
                        .map_err(|err| InternalError::from_source(Box::new(err)))?;
                    arg_builder = arg_builder.with_consensus(consensus);
                }
                _ => {
                    return Err(InternalError::with_message(format!(
                        "Received unknown argument: {}",
                        key
                    )))
                }
            }
        }

        arg_builder
            .build()
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }
}

/// Parse a service argument into a list. Check if the argument is in json or csv format
/// and return the list of strings. An error is returned if json fmt cannot be parsed.
fn parse_list(values_list: &str) -> Result<Vec<String>, String> {
    if values_list.starts_with('[') {
        serde_json::from_str(values_list).map_err(|err| err.to_string())
    } else {
        Ok(values_list
            .split(',')
            .map(String::from)
            .collect::<Vec<String>>())
    }
}
