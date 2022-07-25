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

use splinter::{
    error::{InternalError, InvalidArgumentError},
    service::{ArgumentsConverter, ServiceId},
};

use super::{EchoArguments, EchoArgumentsBuilder};

pub struct EchoArgumentsVecConverter {}

impl ArgumentsConverter<EchoArguments, Vec<(String, String)>> for EchoArgumentsVecConverter {
    fn to_right(&self, left: EchoArguments) -> Result<Vec<(String, String)>, InternalError> {
        let arguments = vec![
            (
                "peer_services".to_string(),
                left.peers()
                    .iter()
                    .map(|service_id| service_id.to_string())
                    .collect::<Vec<String>>()
                    .join(","),
            ),
            (
                "frequency".to_string(),
                left.frequency().as_secs().to_string(),
            ),
            ("jitter".to_string(), left.jitter().as_secs().to_string()),
            ("error_rate".to_string(), left.error_rate().to_string()),
        ];
        Ok(arguments)
    }

    fn to_left(&self, right: Vec<(String, String)>) -> Result<EchoArguments, InternalError> {
        let mut arg_builder = EchoArgumentsBuilder::new();

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
                "frequency" => {
                    let frequency =
                        std::time::Duration::from_secs(value.parse::<u64>().map_err(|_| {
                            InternalError::with_message("Unable to convert frequency to u64".into())
                        })?);
                    arg_builder = arg_builder.with_frequency(frequency);
                }
                "jitter" => {
                    let jitter =
                        std::time::Duration::from_secs(value.parse::<u64>().map_err(|_| {
                            InternalError::with_message("Unable to convert jitter to u64".into())
                        })?);
                    arg_builder = arg_builder.with_jitter(jitter);
                }
                "error_rate" => {
                    let error_rate = value.parse::<f32>().map_err(|_| {
                        InternalError::with_message("Unable to convert error_rate to f32".into())
                    })?;
                    arg_builder = arg_builder.with_error_rate(error_rate);
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
