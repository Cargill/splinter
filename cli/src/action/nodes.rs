// Copyright 2019 Cargill Incorporated
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

use clap::{ArgMatches, Values as ArgValues};
use flexi_logger::ReconfigurationHandle;
use reqwest::{Client, RequestBuilder, Response};
use serde_json::Value as JsonValue;
use splinter::node_registry::Node;

use super::Action;
use crate::error::CliError;

const ENDPOINT: &str = "/nodes";
pub const FILE_ARG: &str = "file";
pub const FORMAT_ARG: &str = "format";
pub const IDENTITIES_ARG: &str = "identities";
pub const REPLACE_ALL_ARG: &str = "replace-all";
pub const URL_ARG: &str = "url";
pub const JSON: &str = "json";
pub const YAML: &str = "yaml";
pub const SUPPORTED_FORMATS: &[&str] = &[JSON, YAML];

pub struct ListAction;

impl Action for ListAction {
    fn run<'a>(
        &mut self,
        arg_matches: Option<&ArgMatches<'a>>,
        _logger_handle: &mut ReconfigurationHandle,
    ) -> Result<(), CliError> {
        let base_url = get_base_url_from_args(arg_matches);
        let endpoint_and_query = concat(ENDPOINT, "");
        let nodes = get_nodes_list(base_url, &endpoint_and_query, vec![])?;
        let display_format = get_format_from_args(arg_matches);
        for node in nodes {
            display_node_in_format(&node, display_format)?;
        }
        Ok(())
    }
}

fn get_nodes_list(
    base_url: &str,
    endpoint_and_query: &str,
    mut nodes: Vec<Node>,
) -> Result<Vec<Node>, CliError> {
    let request_url = concat(base_url, endpoint_and_query);

    let request = Client::new().get(&request_url);
    let response = perform_request(request)?;

    let mut response_as_json: JsonValue = parse_response(response)?;
    let mut new_nodes = parse_nodes_from_list_response_json(&mut response_as_json)?;
    nodes.append(&mut new_nodes);

    if let Some(next_query) = get_next_query_from_list_response_json(&response_as_json)? {
        get_nodes_list(base_url, next_query, nodes)
    } else {
        Ok(nodes)
    }
}

pub struct ShowAction;

impl Action for ShowAction {
    fn run<'a>(
        &mut self,
        arg_matches: Option<&ArgMatches<'a>>,
        _logger_handle: &mut ReconfigurationHandle,
    ) -> Result<(), CliError> {
        let base_url = get_base_url_from_args(arg_matches);
        let identity = get_required_single_value_arg(arg_matches, "identity")?;
        let endpoint = make_nodes_endpoint_with_identity(identity);
        let request_url = concat(base_url, &endpoint);

        let request = Client::new().get(&request_url);
        let response = perform_request(request)?;

        let format = get_format_from_args(arg_matches);
        let node: Node = parse_response(response)?;
        display_node_in_format(&node, format)
    }
}

pub struct AddAction;

impl Action for AddAction {
    fn run<'a>(
        &mut self,
        arg_matches: Option<&ArgMatches<'a>>,
        _logger_handle: &mut ReconfigurationHandle,
    ) -> Result<(), CliError> {
        let base_url = get_base_url_from_args(arg_matches);
        let request_url = concat(base_url, ENDPOINT);

        let file_path = get_required_single_value_arg(arg_matches, FILE_ARG)?;
        let file = open_file(file_path)?;
        let nodes = read_nodes_from_file(&file)?;

        let client = Client::new();
        for node in nodes {
            let node_as_json_bytes = node_to_json_bytes(&node)?;
            let request = client.post(&request_url).body(node_as_json_bytes);
            if let Err(err) = perform_request(request) {
                println!("failed to add node ({:?}): {}", node, err);
            }
        }

        Ok(())
    }
}

pub struct UpdateAction;

impl Action for UpdateAction {
    fn run<'a>(
        &mut self,
        arg_matches: Option<&ArgMatches<'a>>,
        _logger_handle: &mut ReconfigurationHandle,
    ) -> Result<(), CliError> {
        let file_path = get_required_single_value_arg(arg_matches, FILE_ARG)?;
        let file = open_file(file_path)?;
        let nodes = read_nodes_from_file(&file)?;

        if nodes.is_empty() {
            println!("file invalid, no node definitions found");
            return Ok(());
        }

        let base_url = get_base_url_from_args(arg_matches);
        let client = Client::new();

        if is_flag_present(arg_matches, REPLACE_ALL_ARG) {
            let request_url = concat(base_url, ENDPOINT);
            let nodes_as_json_bytes = array_of_nodes_to_json_bytes(&nodes)?;
            let request = client.put(&request_url).body(nodes_as_json_bytes);
            if let Err(err) = perform_request(request) {
                println!("failed to replace all nodes in registry: {}", err);
            }
        } else {
            for node in nodes {
                let endpoint = make_nodes_endpoint_with_identity(&node.identity);
                let request_url = concat(base_url, &endpoint);
                let node_as_json_bytes = node_to_json_bytes(&node)?;
                let request = client.put(&request_url).body(node_as_json_bytes);
                if let Err(err) = perform_request(request) {
                    println!("failed to update node ({:?}): {}", node, err);
                }
            }
        }

        Ok(())
    }
}

pub struct RemoveAction;

impl Action for RemoveAction {
    fn run<'a>(
        &mut self,
        arg_matches: Option<&ArgMatches<'a>>,
        _logger_handle: &mut ReconfigurationHandle,
    ) -> Result<(), CliError> {
        let base_url = get_base_url_from_args(arg_matches);
        let identities = get_required_multiple_value_arg(arg_matches, IDENTITIES_ARG)?;

        let client = Client::new();
        for identity in identities {
            let endpoint = make_nodes_endpoint_with_identity(identity);
            let request_url = concat(base_url, &endpoint);

            let request = client.delete(&request_url);
            if let Err(err) = perform_request(request) {
                println!("failed to remove node with identity {}: {}", identity, err);
            }
        }

        Ok(())
    }
}

// --- Parse CLI arguments ---

fn get_base_url_from_args<'a>(arg_matches: Option<&'a ArgMatches<'a>>) -> &'a str {
    arg_matches
        .and_then(|args| args.value_of(URL_ARG))
        .expect("should have default URL arg")
}

fn get_format_from_args<'a>(arg_matches: Option<&'a ArgMatches<'a>>) -> &'a str {
    arg_matches
        .and_then(|args| args.value_of(FORMAT_ARG))
        .expect("should have default format arg")
}

fn get_required_single_value_arg<'a>(
    arg_matches: Option<&'a ArgMatches<'a>>,
    arg_name: &str,
) -> Result<&'a str, CliError> {
    arg_matches
        .and_then(|args| args.value_of(arg_name))
        .ok_or_else(|| CliError::MissingArg(arg_name.into()))
}

fn get_required_multiple_value_arg<'a>(
    arg_matches: Option<&'a ArgMatches<'a>>,
    arg_name: &str,
) -> Result<ArgValues<'a>, CliError> {
    arg_matches
        .and_then(|args| args.values_of(arg_name))
        .ok_or_else(|| CliError::MissingArg(arg_name.into()))
}

fn is_flag_present<'a>(arg_matches: Option<&'a ArgMatches<'a>>, flag: &str) -> bool {
    arg_matches
        .map(|args| args.is_present(flag))
        .unwrap_or(false)
}

// --- Build requests ---

fn concat(url: &str, endpoint: &str) -> String {
    format!("{}{}", url, endpoint)
}

fn make_nodes_endpoint_with_identity(identity: &str) -> String {
    format!("{}/{}", ENDPOINT, identity)
}

fn open_file(file_path: &str) -> Result<File, CliError> {
    File::open(file_path)
        .map_err(|err| CliError::ActionError(format!("failed to open file {}: {}", file_path, err)))
}

fn read_nodes_from_file(file: &File) -> Result<Vec<Node>, CliError> {
    serde_yaml::from_reader(file)
        .or_else(|_| serde_json::from_reader(file))
        .map_err(|err| {
            CliError::ActionError(format!("failed to parse file as JSON or YAML: {}", err))
        })
}

fn node_to_json_bytes(node: &Node) -> Result<Vec<u8>, CliError> {
    serde_json::to_vec(node).map_err(|err| {
        CliError::ActionError(format!("failed to serialize node as JSON bytes: {}", err))
    })
}

fn array_of_nodes_to_json_bytes(nodes: &[Node]) -> Result<Vec<u8>, CliError> {
    serde_json::to_vec(nodes).map_err(|err| {
        CliError::ActionError(format!("failed to serialize nodes as JSON bytes: {}", err))
    })
}

// --- Perform requests ---

fn perform_request(request: RequestBuilder) -> Result<Response, CliError> {
    let result = request.send();
    check_result(result)
}

fn check_result(result: reqwest::Result<Response>) -> Result<Response, CliError> {
    result
        .map_err(|err| CliError::ActionError(format!("request failed: {}", err)))?
        .error_for_status()
        .map_err(|err| CliError::ActionError(format!("received error status code: {}", err)))
}

// --- Parse responses ---

fn parse_response<T: serde::de::DeserializeOwned>(mut response: Response) -> Result<T, CliError> {
    response
        .json()
        .map_err(|err| CliError::ActionError(format!("failed to parse response: {}", err)))
}

fn parse_nodes_from_list_response_json(
    response_json: &mut JsonValue,
) -> Result<Vec<Node>, CliError> {
    let response_data = response_json
        .get_mut("data")
        .ok_or_else(|| CliError::ActionError("invalid list response; no 'data' field".into()))?
        .take();
    serde_json::from_value(response_data).map_err(|err| {
        CliError::ActionError(format!("failed to parse response data as nodes: {}", err))
    })
}

fn get_next_query_from_list_response_json(
    response_json: &JsonValue,
) -> Result<Option<&str>, CliError> {
    let paging = response_json
        .get("paging")
        .ok_or_else(|| CliError::ActionError("invalid list response; no 'paging' field".into()))?;
    let offset = paging
        .get("offset")
        .ok_or_else(|| {
            CliError::ActionError("invalid list response; no 'offset' in paging data".into())
        })?
        .as_u64()
        .ok_or_else(|| {
            CliError::ActionError("invalid list response; 'offset' is not a valid number".into())
        })?;
    let limit = paging
        .get("limit")
        .ok_or_else(|| {
            CliError::ActionError("invalid list response; no 'limit' in paging data".into())
        })?
        .as_u64()
        .ok_or_else(|| {
            CliError::ActionError("invalid list response; 'limit' is not a valid number".into())
        })?;
    let total = paging
        .get("total")
        .ok_or_else(|| {
            CliError::ActionError("invalid list response; no 'total' in paging data".into())
        })?
        .as_u64()
        .ok_or_else(|| {
            CliError::ActionError("invalid list response; 'total' is not a valid number".into())
        })?;
    if offset + limit >= total {
        Ok(None)
    } else {
        Ok(Some(
            paging
                .get("next")
                .ok_or_else(|| {
                    CliError::ActionError("invalid list response; no 'next' in paging data".into())
                })?
                .as_str()
                .ok_or_else(|| {
                    CliError::ActionError(
                        "invalid list response; 'next' is not a valid string".into(),
                    )
                })?,
        ))
    }
}

// --- Display nodes ---

fn display_node_in_format(node: &Node, format: &str) -> Result<(), CliError> {
    if format == JSON {
        print_node_as_json(node);
    } else if format == YAML {
        print_node_as_yaml(node);
    } else {
        return Err(CliError::ActionError(format!(
            "invalid format specified: {}",
            format
        )));
    }
    Ok(())
}

fn print_node_as_json(node: &Node) {
    println!("{}", node_to_json_string(node));
}

fn print_node_as_yaml(node: &Node) {
    println!("{}", node_to_yaml_string(node));
}

fn node_to_json_string(node: &Node) -> String {
    serde_json::to_string_pretty(node)
        .unwrap_or_else(|err| format!("failed to convert node to JSON string: {}", err))
}

fn node_to_yaml_string(node: &Node) -> String {
    serde_yaml::to_string(node)
        .unwrap_or_else(|err| format!("failed to convert node to YAML string: {}", err))
}
