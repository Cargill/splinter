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

use super::Action;
use crate::error::CliError;
use crate::store::node::{FileBackedNodeStore, Node, NodeStore, NodeStoreError};

use clap::ArgMatches;
use reqwest::Url;

pub struct AddNodeAliasAction;

impl Action for AddNodeAliasAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let args = arg_matches.ok_or_else(|| CliError::RequiresArgs)?;
        let alias = match args.value_of("alias") {
            Some(alias) => alias,
            None => return Err(CliError::ActionError("Alias is required".into())),
        };
        let node_id = match args.value_of("node_id") {
            Some(node_id) => node_id,
            None => return Err(CliError::ActionError("Node ID is required".into())),
        };

        let endpoint = match args.value_of("endpoint") {
            Some(endpoint) => endpoint,
            None => return Err(CliError::ActionError("Endpoint is required".into())),
        };

        validate_node_endpont(&endpoint)?;

        let node_store = get_node_store();

        if !args.is_present("force") && node_store.get_node(&alias)?.is_some() {
            return Err(CliError::ActionError(format!(
                "Alias {} is already in use",
                alias
            )));
        }
        let node = Node::new(alias, node_id, endpoint);

        node_store.add_node(&node)?;

        Ok(())
    }
}

pub struct ShowNodeAliasAction;

impl Action for ShowNodeAliasAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let args = arg_matches.ok_or_else(|| CliError::RequiresArgs)?;
        let alias = match args.value_of("alias") {
            Some(alias) => alias,
            None => return Err(CliError::ActionError("Alias is required".into())),
        };

        let node_store = get_node_store();

        let node = node_store.get_node(alias)?;

        if let Some(node) = node {
            println!(
                "Node alias: {}, ID: {}, endpoint: {}",
                node.alias(),
                node.node_id(),
                node.endpoint()
            )
        } else {
            println!("Alias not found {}", alias)
        }

        Ok(())
    }
}

pub struct ListNodeAliasAction;

impl Action for ListNodeAliasAction {
    fn run<'a>(&mut self, _: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let node_store = get_node_store();

        let nodes = node_store.list_nodes()?;

        if nodes.is_empty() {
            println!("No node alias have been set yet");
        } else {
            nodes.iter().for_each(|node| {
                println!(
                    "Node alias: {}, ID: {}, endpoint: {}",
                    node.alias(),
                    node.node_id(),
                    node.endpoint()
                )
            })
        }
        Ok(())
    }
}

pub struct DeleteNodeAliasAction;

impl Action for DeleteNodeAliasAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let args = arg_matches.ok_or_else(|| CliError::RequiresArgs)?;

        let alias = match args.value_of("alias") {
            Some(alias) => alias,
            None => return Err(CliError::ActionError("Alias is required".into())),
        };

        let node_store = get_node_store();

        node_store.delete_node(alias)?;

        Ok(())
    }
}

pub fn get_node_store() -> FileBackedNodeStore {
    FileBackedNodeStore::default()
}

fn validate_node_endpont(endpoint: &str) -> Result<(), CliError> {
    if let Err(err) = Url::parse(endpoint) {
        Err(CliError::ActionError(format!(
            "{} is not a valid url: {}",
            endpoint, err
        )))
    } else {
        Ok(())
    }
}

impl From<NodeStoreError> for CliError {
    fn from(err: NodeStoreError) -> Self {
        CliError::ActionError(format!("Failed to perform node operation: {}", err))
    }
}
