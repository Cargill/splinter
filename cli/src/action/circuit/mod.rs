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

mod api;
mod builder;
mod payload;
#[cfg(feature = "circuit-template")]
pub mod template;

#[cfg(feature = "circuit-template")]
use std::collections::HashMap;
use std::fs::File;

use clap::ArgMatches;
use serde::Deserialize;
use splinter::admin::messages::{CreateCircuit, SplinterService};

use crate::error::CliError;
#[cfg(feature = "circuit-template")]
use crate::template::CircuitTemplate;

use super::api::SplinterRestClientBuilder;
use super::{
    msg_from_io_error, read_private_key, Action, DEFAULT_SPLINTER_REST_API_URL,
    SPLINTER_REST_API_URL_ENV,
};

#[cfg(feature = "splinter-cli-jwt")]
use super::create_cylinder_jwt_auth;

use api::{CircuitServiceSlice, CircuitSlice};
pub(crate) use builder::CreateCircuitMessageBuilder;
use payload::make_signed_payload;

pub struct CircuitProposeAction;

impl Action for CircuitProposeAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let args = arg_matches.ok_or(CliError::RequiresArgs)?;

        let mut builder = CreateCircuitMessageBuilder::new();

        if let Some(node_file) = args.value_of("node_file") {
            for node in load_nodes_from_file(node_file)? {
                builder.add_node(&node.identity, &node.endpoints)?;
            }
        }

        if let Some(nodes) = args.values_of("node") {
            for node_argument in nodes {
                let (node, endpoints) = parse_node_argument(node_argument)?;
                builder.add_node(&node, &endpoints)?;
            }
        }

        #[cfg(feature = "circuit-template")]
        {
            if let Some(template_name) = args.value_of("template") {
                let mut template = CircuitTemplate::load(template_name)?;

                let user_args = match args.values_of("template_arg") {
                    Some(template_args) => {
                        parse_template_args(&template_args.collect::<Vec<&str>>())?
                    }
                    None => HashMap::new(),
                };
                template.add_arguments(&user_args);
                template.set_nodes(&builder.get_node_ids());

                template.apply_to_builder(&mut builder)?;
            }
        }

        if let Some(services) = args.values_of("service") {
            for service in services {
                let (service_id, allowed_nodes) = parse_service(service)?;
                builder.add_service(&service_id, &allowed_nodes)?;
            }
        }

        if let Some(service_arguments) = args.values_of("service_argument") {
            for service_argument in service_arguments {
                let (service_id_match, argument) = parse_service_argument(service_argument)?;
                builder.apply_service_arguments(&service_id_match, &argument)?;
            }
        }

        if let Some(service_peer_group) = args.values_of("service_peer_group") {
            for peer_group in service_peer_group {
                let group = parse_service_peer_group(peer_group)?;
                builder.apply_peer_services(&group)?;
            }
        }

        #[cfg(feature = "circuit-auth-type")]
        #[allow(clippy::single_match)]
        match args.value_of("authorization_type") {
            Some(auth_type) => builder.set_authorization_type(auth_type)?,
            None => (),
        }

        if let Some(management_type) = args.value_of("management_type") {
            builder.set_management_type(management_type);
        }

        if let Some(mut application_metadata) = args.values_of("metadata") {
            let encoding = args.value_of("metadata_encoding").unwrap_or("string");
            match encoding {
                "string" => {
                    if application_metadata.len() > 1 {
                        return Err(CliError::ActionError(
                            "Multiple metadata values with encoding 'string' is not allowed".into(),
                        ));
                    }
                    if let Some(metadata) = application_metadata.next() {
                        builder.set_application_metadata(metadata.as_bytes());
                    }
                }
                "json" => {
                    let mut json_string = "{".to_string();
                    for metadata in application_metadata {
                        let values = parse_application_metadata_json(metadata)?;
                        json_string = format!("{}{},", json_string, values);
                    }
                    json_string.pop();
                    json_string.push('}');

                    builder.set_application_metadata(&json_string.as_bytes());
                }
                _ => {
                    return Err(CliError::ActionError(format!(
                        "Metadata encoding '{}' is not supported",
                        encoding
                    )))
                }
            }
        }

        if let Some(service_types) = args.values_of("service_type") {
            for service_type_arg in service_types {
                let (service_id_match, service_type) =
                    parse_service_type_argument(service_type_arg)?;
                builder.apply_service_type(&service_id_match, &service_type);
            }
        }

        if let Some(comments) = args.value_of("comments") {
            builder.set_comments(comments);
        }

        let create_circuit = builder.build()?;

        let circuit_slice = CircuitSlice::from(&create_circuit);

        if !args.is_present("dry_run") {
            let url = args
                .value_of("url")
                .map(ToOwned::to_owned)
                .or_else(|| std::env::var(SPLINTER_REST_API_URL_ENV).ok())
                .unwrap_or_else(|| DEFAULT_SPLINTER_REST_API_URL.to_string());

            let key = args.value_of("key");

            let mut builder = SplinterRestClientBuilder::new();
            builder = builder.with_url(url);

            #[cfg(feature = "splinter-cli-jwt")]
            {
                builder = builder.with_auth(create_cylinder_jwt_auth(key)?);
            }

            let client = builder.build()?;

            let requester_node = client.get_node_status()?.node_id;
            let private_key_hex = read_private_key(&key.unwrap_or("./splinter.priv"))?;

            let signed_payload =
                make_signed_payload(&requester_node, &private_key_hex, create_circuit)?;
            client.submit_admin_payload(signed_payload)?;

            info!("The circuit proposal was submited successfully");
        }

        info!("{}", circuit_slice);

        Ok(())
    }
}

#[derive(Deserialize)]
struct Node {
    #[serde(alias = "node_id")]
    identity: String,
    endpoints: Vec<String>,
}

fn load_nodes_from_file(node_file: &str) -> Result<Vec<Node>, CliError> {
    if node_file.starts_with("http://") || node_file.starts_with("https://") {
        load_nodes_from_remote(node_file)
    } else {
        load_nodes_from_local(node_file)
    }
}

fn load_nodes_from_remote(url: &str) -> Result<Vec<Node>, CliError> {
    let bytes = reqwest::blocking::get(url)
        .and_then(|response| response.error_for_status())
        .map_err(|err| {
            CliError::ActionError(format!(
                "Failed to fetch remote node file from {}: {}",
                url, err
            ))
        })?
        .bytes()
        .map_err(|err| {
            CliError::ActionError(format!(
                "Failed to get bytes from remote node file HTTP response: {}",
                err
            ))
        })?;
    serde_yaml::from_slice(&bytes).map_err(|_| {
        CliError::ActionError(
            "Failed to deserialize remote node file: Not a valid YAML sequence of nodes".into(),
        )
    })
}

fn load_nodes_from_local(node_file: &str) -> Result<Vec<Node>, CliError> {
    let path = if node_file.starts_with("file://") {
        node_file.split_at(7).1
    } else {
        node_file
    };
    let file = File::open(path).map_err(|err| {
        CliError::EnvironmentError(format!(
            "Unable to open node file '{}': {}",
            path,
            msg_from_io_error(err)
        ))
    })?;
    serde_yaml::from_reader(file).map_err(|_| {
        CliError::ActionError(format!(
            "Failed to read node file '{}': Not a valid YAML sequence of nodes",
            path
        ))
    })
}

fn parse_node_argument(node_argument: &str) -> Result<(String, Vec<String>), CliError> {
    let mut iter = node_argument.split("::");

    let node_id = iter
        .next()
        .expect("str::split cannot return an empty iterator")
        .to_string();
    if node_id.is_empty() {
        return Err(CliError::ActionError(
            "Empty '--node' argument detected".into(),
        ));
    }

    let endpoints = iter
        .next()
        .ok_or_else(|| CliError::ActionError(format!("Missing endpoints for node '{}'", node_id)))?
        .to_string();
    if endpoints.is_empty() {
        return Err(CliError::ActionError(format!(
            "No endpoints detected for node '{}'",
            node_id
        )));
    }

    let endpoints = endpoints
        .split(',')
        .map(|endpoint| {
            if endpoint.is_empty() {
                Err(CliError::ActionError(format!(
                    "Empty endpoints detected for node '{}'",
                    node_id
                )))
            } else {
                Ok(endpoint.to_string())
            }
        })
        .collect::<Result<_, _>>()?;

    Ok((node_id, endpoints))
}

fn parse_service(service: &str) -> Result<(String, Vec<String>), CliError> {
    let mut iter = service.split("::");

    let service_id = iter
        .next()
        .expect("str::split cannot return an empty iterator")
        .to_string();
    if service_id.is_empty() {
        return Err(CliError::ActionError(
            "Empty '--service' argument detected".into(),
        ));
    }

    let allowed_nodes = iter
        .next()
        .ok_or_else(|| {
            CliError::ActionError(format!(
                "Missing allowed nodes for service '{}'",
                service_id
            ))
        })?
        .split(',')
        .map(|allowed_node| {
            if allowed_node.is_empty() {
                Err(CliError::ActionError(format!(
                    "Empty allowed node detected for service '{}'",
                    service_id
                )))
            } else {
                Ok(allowed_node.to_string())
            }
        })
        .collect::<Result<Vec<String>, CliError>>()?;

    Ok((service_id, allowed_nodes))
}

fn parse_service_peer_group(peer_group: &str) -> Result<Vec<&str>, CliError> {
    peer_group
        .split(',')
        .map(|peer| {
            if peer.is_empty() {
                Err(CliError::ActionError(
                    "Empty service_id detected in '--service-peer-group' list".into(),
                ))
            } else {
                Ok(peer)
            }
        })
        .collect::<Result<_, _>>()
}

fn parse_application_metadata_json(metadata: &str) -> Result<String, CliError> {
    let mut iter = metadata.split('=');

    let key = iter
        .next()
        .expect("str::split cannot return an empty iterator")
        .to_string();
    if key.is_empty() {
        return Err(CliError::ActionError(
            "Empty '--metadata' argument detected".into(),
        ));
    }

    let mut value = iter
        .next()
        .ok_or_else(|| CliError::ActionError(format!("Missing value for metadata key '{}'", key)))?
        .to_string();
    if key.is_empty() {
        return Err(CliError::ActionError(format!(
            "Empty value detected for metadata key '{}'",
            key
        )));
    }

    // If the value isn't an array or object, add quotes to make it a valid JSON string
    if !value.contains('[') && !value.contains('{') {
        value = format!("\"{}\"", value);
    }

    Ok(format!("\"{}\":{}", key, value))
}

#[cfg(feature = "circuit-template")]
fn parse_template_args(args: &[&str]) -> Result<HashMap<String, String>, CliError> {
    args.iter().try_fold(HashMap::new(), |mut acc, arg| {
        let mut iter = arg.split('=');
        let key = iter
            .next()
            .ok_or_else(|| {
                CliError::ActionError(format!(
                    "Invalid template argument. Expected value in form <key>=<value> found {}",
                    arg
                ))
            })?
            .to_string();

        let value = iter
            .next()
            .ok_or_else(|| {
                CliError::ActionError(format!(
                    "Invalid template argument. Expected value in form <key>=<value> found {}",
                    arg
                ))
            })?
            .to_string();

        if key.is_empty() || value.is_empty() {
            return Err(CliError::ActionError(format!(
                "Invalid template argument. Key or value cannot be empty.\
                 Expected value in form <key>=<value> found {}",
                arg
            )));
        }

        acc.insert(key, value);
        Ok(acc)
    })
}

fn parse_service_argument(service_argument: &str) -> Result<(String, (String, String)), CliError> {
    let mut iter = service_argument.split("::");

    let service_id = iter
        .next()
        .expect("str::split cannot return an empty iterator")
        .to_string();
    if service_id.is_empty() {
        return Err(CliError::ActionError(
            "Empty '--service-argument' argument detected".into(),
        ));
    }

    let arguments = iter
        .next()
        .ok_or_else(|| {
            CliError::ActionError(format!(
                "Missing key/value in service argument for '{}'",
                service_id,
            ))
        })?
        .to_string();

    let mut argument_iter = arguments.split('=');

    let key = argument_iter
        .next()
        .expect("str::split cannot return an empty iterator")
        .to_string();
    if key.is_empty() {
        return Err(CliError::ActionError(format!(
            "Empty key/value detected in service argument for '{}'",
            service_id
        )));
    }

    let value = argument_iter.next().ok_or_else(|| {
        CliError::ActionError(format!(
            "Missing value in service argument '{}::{}'",
            service_id, key,
        ))
    })?;
    if value.is_empty() {
        return Err(CliError::ActionError(format!(
            "Empty value detected in service argument '{}::{}'",
            service_id, key,
        )));
    }

    Ok((service_id, (key, format!("{:?}", vec![value]))))
}

fn parse_service_type_argument(service_type: &str) -> Result<(String, String), CliError> {
    let mut iter = service_type.split("::");

    let service_id = iter
        .next()
        .expect("str::split cannot return an empty iterator")
        .to_string();
    if service_id.is_empty() {
        return Err(CliError::ActionError(
            "Empty '--service-type' argument detected".into(),
        ));
    }

    let service_type = iter
        .next()
        .ok_or_else(|| CliError::ActionError(format!("Missing service type for '{}'", service_id)))?
        .to_string();
    if service_type.is_empty() {
        return Err(CliError::ActionError(format!(
            "Empty service type detected for '{}'",
            service_id
        )));
    }

    Ok((service_id, service_type))
}

impl From<&CreateCircuit> for CircuitSlice {
    fn from(circuit: &CreateCircuit) -> Self {
        Self {
            id: circuit.circuit_id.clone(),
            members: circuit
                .members
                .iter()
                .map(|member| member.node_id.clone())
                .collect(),
            roster: circuit
                .roster
                .iter()
                .map(|service| service.into())
                .collect(),
            management_type: circuit.circuit_management_type.clone(),
        }
    }
}

impl From<&SplinterService> for CircuitServiceSlice {
    fn from(service: &SplinterService) -> Self {
        Self {
            service_id: service.service_id.clone(),
            service_type: service.service_type.clone(),
            allowed_nodes: service.allowed_nodes.clone(),
            arguments: service.arguments.iter().cloned().collect(),
        }
    }
}

enum Vote {
    Accept,
    Reject,
}

struct CircuitVote {
    circuit_id: String,
    circuit_hash: String,
    vote: Vote,
}

pub struct CircuitVoteAction;

impl Action for CircuitVoteAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let args = arg_matches.ok_or(CliError::RequiresArgs)?;
        let url = args
            .value_of("url")
            .map(ToOwned::to_owned)
            .or_else(|| std::env::var(SPLINTER_REST_API_URL_ENV).ok())
            .unwrap_or_else(|| DEFAULT_SPLINTER_REST_API_URL.to_string());
        let key = args.value_of("private_key_file");

        let circuit_id = args
            .value_of("circuit_id")
            .ok_or_else(|| CliError::ActionError("'circuit-id' argument is required".into()))?;

        // accept or reject must be present
        let vote = {
            if args.is_present("accept") {
                Vote::Accept
            } else {
                Vote::Reject
            }
        };

        vote_on_circuit_proposal(&url, key, circuit_id, vote)
    }
}

fn vote_on_circuit_proposal(
    url: &str,
    key: Option<&str>,
    circuit_id: &str,
    vote: Vote,
) -> Result<(), CliError> {
    let mut builder = SplinterRestClientBuilder::new();
    builder = builder.with_url(url.to_string());

    #[cfg(feature = "splinter-cli-jwt")]
    {
        builder = builder.with_auth(create_cylinder_jwt_auth(key)?);
    }

    let client = builder.build()?;

    let private_key_hex = read_private_key(key.unwrap_or("splinter"))?;

    let requester_node = client.get_node_status()?.node_id;
    let proposal = client.fetch_proposal(circuit_id)?;

    if let Some(proposal) = proposal {
        let circuit_vote = CircuitVote {
            circuit_id: circuit_id.into(),
            circuit_hash: proposal.circuit_hash,
            vote,
        };
        let signed_payload = make_signed_payload(&requester_node, &private_key_hex, circuit_vote)?;
        client.submit_admin_payload(signed_payload)
    } else {
        Err(CliError::ActionError(format!(
            "Proposal for circuit '{}' does not exist",
            circuit_id
        )))
    }
}

pub struct CircuitListAction;

impl Action for CircuitListAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let url = arg_matches
            .and_then(|args| args.value_of("url"))
            .map(ToOwned::to_owned)
            .or_else(|| std::env::var(SPLINTER_REST_API_URL_ENV).ok())
            .unwrap_or_else(|| DEFAULT_SPLINTER_REST_API_URL.to_string());

        let filter = arg_matches.and_then(|args| args.value_of("member"));

        let format = arg_matches
            .and_then(|args| {
                if let Some(val) = args.value_of("hidden_format") {
                    Some(val)
                } else {
                    args.value_of("format")
                }
            })
            .unwrap_or("human");

        #[cfg(feature = "splinter-cli-jwt")]
        let key = arg_matches.and_then(|args| args.value_of("private_key_file"));

        list_circuits(
            &url,
            filter,
            format,
            #[cfg(feature = "splinter-cli-jwt")]
            key,
        )
    }
}

fn list_circuits(
    url: &str,
    filter: Option<&str>,
    format: &str,
    #[cfg(feature = "splinter-cli-jwt")] key: Option<&str>,
) -> Result<(), CliError> {
    let mut builder = SplinterRestClientBuilder::new();
    builder = builder.with_url(url.to_string());

    #[cfg(feature = "splinter-cli-jwt")]
    {
        builder = builder.with_auth(create_cylinder_jwt_auth(key)?);
    }

    let client = builder.build()?;

    let circuits = client.list_circuits(filter)?;
    let mut data = Vec::new();
    data.push(vec![
        "ID".to_string(),
        "MANAGEMENT".to_string(),
        "MEMBERS".to_string(),
    ]);
    circuits.data.iter().for_each(|circuit| {
        let members = circuit.members.join(";");
        data.push(vec![
            circuit.id.to_string(),
            circuit.management_type.to_string(),
            members,
        ]);
    });

    if format == "csv" {
        for row in data {
            println!("{}", row.join(","))
        }
    } else {
        print_table(data);
    }
    Ok(())
}

pub struct CircuitShowAction;

impl Action for CircuitShowAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let args = arg_matches.ok_or(CliError::RequiresArgs)?;

        let url = args
            .value_of("url")
            .map(ToOwned::to_owned)
            .or_else(|| std::env::var(SPLINTER_REST_API_URL_ENV).ok())
            .unwrap_or_else(|| DEFAULT_SPLINTER_REST_API_URL.to_string());
        let circuit_id = args
            .value_of("circuit")
            .ok_or_else(|| CliError::ActionError("'circuit' argument is required".to_string()))?;

        let format = if let Some(val) = args.value_of("hidden_format") {
            val
        } else {
            args.value_of("format").unwrap_or("human")
        };

        #[cfg(feature = "splinter-cli-jwt")]
        let key = arg_matches.and_then(|args| args.value_of("private_key_file"));

        show_circuit(
            &url,
            circuit_id,
            format,
            #[cfg(feature = "splinter-cli-jwt")]
            key,
        )
    }
}

fn show_circuit(
    url: &str,
    circuit_id: &str,
    format: &str,
    #[cfg(feature = "splinter-cli-jwt")] key: Option<&str>,
) -> Result<(), CliError> {
    let mut builder = SplinterRestClientBuilder::new();
    builder = builder.with_url(url.to_string());

    #[cfg(feature = "splinter-cli-jwt")]
    {
        builder = builder.with_auth(create_cylinder_jwt_auth(key)?);
    }

    let client = builder.build()?;

    let circuit = client.fetch_circuit(circuit_id)?;
    let mut print_circuit = false;
    let mut print_proposal = false;
    if let Some(circuit) = circuit {
        print_circuit = true;
        match format {
            "json" => println!(
                "\n {}",
                serde_json::to_string(&circuit).map_err(|err| CliError::ActionError(format!(
                    "Cannot format circuit into json: {}",
                    err
                )))?
            ),
            "yaml" => println!(
                "{}",
                serde_yaml::to_string(&circuit).map_err(|err| CliError::ActionError(format!(
                    "Cannot format circuit into yaml: {}",
                    err
                )))?
            ),
            _ => println!("{}", circuit),
        }
    }

    let proposal = client.fetch_proposal(circuit_id)?;

    if let Some(proposal) = proposal {
        print_proposal = true;
        match format {
            "json" => println!(
                "\n {}",
                serde_json::to_string(&proposal).map_err(|err| CliError::ActionError(format!(
                    "Cannot format proposal into json: {}",
                    err
                )))?
            ),
            "yaml" => println!(
                "{}",
                serde_yaml::to_string(&proposal).map_err(|err| CliError::ActionError(format!(
                    "Cannot format proposal into yaml: {}",
                    err
                )))?
            ),
            _ => println!("{}", proposal),
        }
    }

    if !print_circuit && !print_proposal {
        return Err(CliError::ActionError(format!(
            "Circuit or proposal for circuit '{}' does not exist",
            circuit_id
        )));
    }

    Ok(())
}

pub struct CircuitProposalsAction;

impl Action for CircuitProposalsAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let url = arg_matches
            .and_then(|args| args.value_of("url"))
            .map(ToOwned::to_owned)
            .or_else(|| std::env::var(SPLINTER_REST_API_URL_ENV).ok())
            .unwrap_or_else(|| DEFAULT_SPLINTER_REST_API_URL.to_string());

        let management_type_filter = arg_matches.and_then(|args| args.value_of("management_type"));

        let member_filter = arg_matches.and_then(|args| args.value_of("member"));

        let format = arg_matches
            .and_then(|args| {
                if let Some(val) = args.value_of("hidden_format") {
                    Some(val)
                } else {
                    args.value_of("format")
                }
            })
            .unwrap_or("human");

        #[cfg(feature = "splinter-cli-jwt")]
        let key = arg_matches.and_then(|args| args.value_of("private_key_file"));

        list_proposals(
            &url,
            management_type_filter,
            member_filter,
            format,
            #[cfg(feature = "splinter-cli-jwt")]
            key,
        )
    }
}

fn list_proposals(
    url: &str,
    management_type_filter: Option<&str>,
    member_filter: Option<&str>,
    format: &str,
    #[cfg(feature = "splinter-cli-jwt")] key: Option<&str>,
) -> Result<(), CliError> {
    let mut builder = SplinterRestClientBuilder::new();
    builder = builder.with_url(url.to_string());

    #[cfg(feature = "splinter-cli-jwt")]
    {
        builder = builder.with_auth(create_cylinder_jwt_auth(key)?);
    }

    let client = builder.build()?;

    let proposals = client.list_proposals(management_type_filter, member_filter)?;
    let mut data = Vec::new();
    data.push(vec![
        "ID".to_string(),
        "MANAGEMENT".to_string(),
        "MEMBERS".to_string(),
        "COMMENTS".to_string(),
    ]);
    proposals.data.iter().for_each(|proposal| {
        let members = proposal
            .circuit
            .members
            .iter()
            .map(|member| member.node_id.to_string())
            .collect::<Vec<String>>()
            .join(";");
        data.push(vec![
            proposal.circuit_id.to_string(),
            proposal.circuit.management_type.to_string(),
            members,
            proposal.circuit.comments.to_string(),
        ]);
    });

    if format == "csv" {
        for row in data {
            println!("{}", row.join(","))
        }
    } else {
        print_table(data);
    }

    Ok(())
}

// Takes a vec of vecs of strings. The first vec should include the title of the columns.
// The max length of each column is calculated and is used as the column with when printing the
// table.
fn print_table(table: Vec<Vec<String>>) {
    let mut max_lengths = Vec::new();

    // find the max lengths of the columns
    for row in table.iter() {
        for (i, col) in row.iter().enumerate() {
            if let Some(length) = max_lengths.get_mut(i) {
                if col.len() > *length {
                    *length = col.len()
                }
            } else {
                max_lengths.push(col.len())
            }
        }
    }

    // print each row with correct column size
    for row in table.iter() {
        let mut col_string = String::from("");
        for (i, len) in max_lengths.iter().enumerate() {
            if let Some(value) = row.get(i) {
                col_string += &format!("{}{} ", value, " ".repeat(*len - value.len()),);
            } else {
                col_string += &" ".repeat(*len);
            }
        }
        println!("{}", col_string);
    }
}
