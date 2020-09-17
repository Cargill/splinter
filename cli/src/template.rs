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

//! Data structure and implementation of the circuit template representation for the CLI.

use std::collections::HashMap;
use std::path::PathBuf;

use splinter::circuit::template::{
    CircuitCreateTemplate, CircuitTemplateError, CircuitTemplateManager, RuleArgument,
    DEFAULT_TEMPLATE_DIR, SPLINTER_CIRCUIT_TEMPLATE_PATH,
};

use crate::action::circuit::CreateCircuitMessageBuilder;
use crate::error::CliError;

const NODES_ARG: &str = "nodes";

/// Representation of a circuit template used in CLI actions.
pub struct CircuitTemplate {
    template: CircuitCreateTemplate,
    arguments: HashMap<String, String>,
}

impl CircuitTemplate {
    /// Lists all available circuit templates found in the default template directory.
    pub fn list_available_templates() -> Result<Vec<(String, PathBuf)>, CliError> {
        let mut paths = Vec::new();
        if let Ok(env_paths) = std::env::var(SPLINTER_CIRCUIT_TEMPLATE_PATH) {
            paths.extend(
                env_paths
                    .split(':')
                    .map(ToOwned::to_owned)
                    .collect::<Vec<String>>(),
            );
        }
        paths.push(DEFAULT_TEMPLATE_DIR.to_string());
        let manager = CircuitTemplateManager::new(&paths);
        let templates = manager.list_available_templates()?;
        Ok(templates)
    }

    /// Loads a YAML circuit template file into a YAML string.
    ///
    /// # Arguments
    ///
    /// * `name` - File name of the circuit template YAML file.
    pub fn load_raw(name: &str) -> Result<String, CliError> {
        let mut paths = Vec::new();
        if let Ok(env_paths) = std::env::var(SPLINTER_CIRCUIT_TEMPLATE_PATH) {
            paths.extend(
                env_paths
                    .split(':')
                    .map(ToOwned::to_owned)
                    .collect::<Vec<String>>(),
            );
        }
        paths.push(DEFAULT_TEMPLATE_DIR.to_string());
        let manager = CircuitTemplateManager::new(&paths);
        let template_yaml = manager.load_raw_yaml(name)?;
        Ok(template_yaml)
    }

    /// Loads a YAML circuit template file and returns a `CircuitTemplate` that can be used to
    /// build `CreateCircuit` messages.
    ///
    /// # Arguments
    ///
    /// * `name` - File name of the circuit template YAML file.
    pub fn load(name: &str) -> Result<Self, CliError> {
        let mut paths = Vec::new();
        if let Ok(env_paths) = std::env::var(SPLINTER_CIRCUIT_TEMPLATE_PATH) {
            paths.extend(
                env_paths
                    .split(':')
                    .map(ToOwned::to_owned)
                    .collect::<Vec<String>>(),
            );
        }
        paths.push(DEFAULT_TEMPLATE_DIR.to_string());
        let manager = CircuitTemplateManager::new(&paths);
        let possible_values = manager.list_available_templates()?;
        if !possible_values.iter().any(|(stem, _)| stem == name) {
            return Err(CliError::ActionError(format!(
                "Template with name {} was not found. Available templates: {:?}",
                name, possible_values
            )));
        }
        let template = manager.load(name)?;
        Ok(CircuitTemplate {
            template,
            arguments: HashMap::new(),
        })
    }

    fn check_missing_required_arguments(&self) -> Vec<String> {
        self.template
            .arguments()
            .iter()
            .filter_map(|template_argument| {
                if template_argument.required()
                    && self.arguments.get(template_argument.name()).is_none()
                {
                    Some(template_argument.name().to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Sets the `nodes` argument, using node IDs, for the `CircuitTemplate`.
    ///
    /// # Arguments
    ///
    /// * `nodes` - List of node IDs from the available template arguments.
    pub fn set_nodes(&mut self, nodes: &[String]) {
        if self
            .template
            .arguments()
            .iter()
            .any(|arg| arg.name() == NODES_ARG)
        {
            self.arguments
                .insert(NODES_ARG.to_string(), nodes.join(","));
        }
    }

    /// Adds additional template argument, represented by a HashMap, to the `CircuitTemplate`.
    ///
    /// # Arguments
    ///
    /// * `user_arguments` - HashMap of arguments to be added to the `CircuitTemplate`.
    pub fn add_arguments(&mut self, user_arguments: &HashMap<String, String>) {
        self.arguments.extend(user_arguments.clone())
    }

    /// Returns a list of `arguments` stored in the `CircuitTemplate`.
    pub fn arguments(&self) -> &[RuleArgument] {
        self.template.arguments()
    }

    /// Updates a `CreateCircuitMessageBuilder` based on the template argument values.
    ///
    /// Applies all `rules` from the circuit template using the data saved in the `arguments` to
    /// a `CreateCircuitMessageBuilder`. Also adds services created from the circuit template to
    /// the returned builder if the `create_services` rule is in the template.
    pub fn apply_to_builder(
        mut self,
        circuit_message_builder: &mut CreateCircuitMessageBuilder,
    ) -> Result<(), CliError> {
        let circuit_builder = circuit_message_builder.create_circuit_builder();

        let missing_args = self.check_missing_required_arguments();
        if !missing_args.is_empty() {
            return Err(CliError::ActionError(format!(
                "Required arguments were not set: {}",
                missing_args.join(", ")
            )));
        }

        for (key, value) in self.arguments.iter() {
            self.template.set_argument_value(key, value)?;
        }

        circuit_message_builder
            .set_create_circuit_builder(&self.template.apply_to_builder(circuit_builder)?);

        Ok(())
    }
}

impl From<CircuitTemplateError> for CliError {
    fn from(err: CircuitTemplateError) -> CliError {
        CliError::ActionError(format!("Failed to process template: {}", err))
    }
}
