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

use std::fs::File;
use std::io::Write;
use std::path::Path;

use clap::ArgMatches;
use splinter::registry::Node;

use crate::error::CliError;

use super::api::SplinterRestClient;
use super::{
    msg_from_io_error, read_private_key, Action, DEFAULT_SPLINTER_REST_API_URL,
    SPLINTER_REST_API_URL_ENV,
};

const DEFAULT_OUTPUT_FILE: &str = "./nodes.yaml";

pub struct RegistryGenerateAction;

impl Action for RegistryGenerateAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let args = arg_matches.ok_or(CliError::RequiresArgs)?;

        let output_file = args.value_of("file").unwrap_or(DEFAULT_OUTPUT_FILE);

        let mut nodes: Vec<Node> = if Path::new(output_file).exists() {
            let file = File::open(output_file).map_err(|err| {
                CliError::ActionError(format!(
                    "Failed to open '{}': {}",
                    output_file,
                    msg_from_io_error(err)
                ))
            })?;
            serde_yaml::from_reader(file).map_err(|_| {
                CliError::ActionError(format!(
                    "Failed to read registry file '{}': Not a valid YAML sequence of nodes",
                    output_file
                ))
            })?
        } else {
            vec![]
        };

        let url = args
            .value_of("status_url")
            .map(ToOwned::to_owned)
            .or_else(|| std::env::var(SPLINTER_REST_API_URL_ENV).ok())
            .unwrap_or_else(|| DEFAULT_SPLINTER_REST_API_URL.to_string());
        let node_status = SplinterRestClient::new(&url).get_node_status()?;

        let keys = args
            .values_of("key_files")
            .ok_or_else(|| CliError::ActionError("One or more key files must be specified".into()))?
            .map(|key_file| read_private_key(key_file))
            .collect::<Result<_, _>>()?;

        let metadata = if let Some(metadata) = args.values_of("metadata") {
            metadata
                .map(|kv| {
                    let mut kv_iter = kv.splitn(2, '=');

                    let key = kv_iter
                        .next()
                        .expect("str::split cannot return an empty iterator")
                        .to_string();
                    if key.is_empty() {
                        return Err(CliError::ActionError(
                            "Empty '--metadata' argument detected".into(),
                        ));
                    }

                    let value = kv_iter
                        .next()
                        .ok_or_else(|| {
                            CliError::ActionError(format!(
                                "Missing value for metadata key '{}'",
                                key
                            ))
                        })?
                        .to_string();
                    if value.is_empty() {
                        return Err(CliError::ActionError(format!(
                            "Empty value detected for metadata key '{}'",
                            key
                        )));
                    }

                    Ok((key, value))
                })
                .collect::<Result<_, _>>()?
        } else {
            Default::default()
        };

        let node = Node {
            identity: node_status.node_id.clone(),
            endpoints: node_status.advertised_endpoints,
            display_name: node_status.display_name,
            keys,
            metadata,
        };

        if let Some(idx) = nodes
            .iter()
            .position(|existing_node| existing_node.identity == node.identity)
        {
            if args.is_present("force") {
                nodes.remove(idx);
            } else {
                return Err(CliError::EnvironmentError(format!(
                    "Node '{}' already exists; must use '--force' to overwrite an existing node",
                    node.identity
                )));
            }
        }

        nodes.push(node);

        let yaml = serde_yaml::to_vec(&nodes).map_err(|err| {
            CliError::ActionError(format!("Cannot format node list into yaml: {}", err))
        })?;

        let mut file = File::create(output_file).map_err(|err| {
            CliError::ActionError(format!(
                "Failed to create or overwrite '{}': {}",
                output_file,
                msg_from_io_error(err)
            ))
        })?;
        file.write_all(&yaml).map_err(|err| {
            CliError::ActionError(format!(
                "Failed to write to file '{}': {}",
                output_file,
                msg_from_io_error(err)
            ))
        })?;
        // Append newline to file
        writeln!(file).map_err(|err| {
            CliError::ActionError(format!(
                "Failed to write to file '{}': {}",
                output_file,
                msg_from_io_error(err)
            ))
        })?;

        info!("Added node '{}' to '{}'", node_status.node_id, output_file);

        Ok(())
    }
}
