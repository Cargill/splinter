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

//! Data structures that manage and use templates to create circuit templates.
//!
//! The public interface includes the structs [`CircuitTemplateManager`], and
//! [`CircuitCreateTemplate`].
//!
//! [`CircuitTemplateManager`]: struct.CircuitTemplateManager.html
//! [`CircuitCreateTemplate`]: struct.CircuitCreateTemplate.html

mod error;
mod rules;
mod yaml_parser;

use std::convert::TryFrom;
use std::env;
use std::path::Path;

pub use error::CircuitTemplateError;

use glob::glob;
pub use rules::RuleArgument;
use rules::Rules;

use yaml_parser::{v1, CircuitTemplate};

pub(self) use crate::admin::messages::{CreateCircuitBuilder, SplinterServiceBuilder};

/// Default file location for circuit templates
pub const DEFAULT_TEMPLATE_DIR: &str = "/usr/share/splinter/circuit-templates";
/// Environment variable for file location for circuit templates
pub const SPLINTER_CIRCUIT_TEMPLATE_PATH: &str = "SPLINTER_CIRCUIT_TEMPLATE_PATH";

/// Manages circuit templates.
///
/// `CircuitTemplateManager` maintains the location of circuit templates, and may be used to
/// list any availabe circuit templates found in the `path` of the `CircuitTemplateManager`.
pub struct CircuitTemplateManager {
    /// Path of the directory containing the circuit template files.
    paths: Vec<String>,
}

impl Default for CircuitTemplateManager {
    /// Constructs a `CircuitTemplateManager` with the `DEFAULT_TEMPLATE_DIR`.
    fn default() -> Self {
        CircuitTemplateManager {
            paths: vec![DEFAULT_TEMPLATE_DIR.to_string()],
        }
    }
}

impl CircuitTemplateManager {
    /// Constructs a `CircuitTemplateManager` with custom `paths` to the circuit templates.
    pub fn new(paths: &[String]) -> CircuitTemplateManager {
        let paths: Vec<String> = paths
            .iter()
            .filter_map(|path| {
                if Path::new(&path).is_dir() {
                    Some(path.to_string())
                } else {
                    None
                }
            })
            .collect();
        CircuitTemplateManager { paths }
    }

    /// Loads the specified YAML circuit template file into a CircuitCreateTemplate.
    ///
    /// # Arguments
    ///
    /// * `name` - file name indicating the circuit template to be loaded.
    pub fn load(&self, name: &str) -> Result<CircuitCreateTemplate, CircuitTemplateError> {
        let path = find_template(name, &self.paths)?;
        CircuitCreateTemplate::from_yaml_file(&path)
    }

    /// Loads the specified YAML circuit template file into a YAML string.
    ///
    /// # Arguments
    ///
    /// * `name` - file name indicating the circuit template to be loaded into a YAML string.
    pub fn load_raw_yaml(&self, name: &str) -> Result<String, CircuitTemplateError> {
        let path = find_template(name, &self.paths)?;
        let template = CircuitTemplate::load_from_file(&path)?;
        debug!("Loading template file from {}", &path);
        match template {
            CircuitTemplate::V1(template) => serde_yaml::to_string(&template).map_err(|err| {
                CircuitTemplateError::new_with_source(
                    "Failed to load template to yaml string",
                    Box::new(err),
                )
            }),
        }
    }

    /// Lists all available circuit templates found in the `paths` of the `CircuitTemplateManager`.
    pub fn list_available_templates(&self) -> Result<Vec<String>, CircuitTemplateError> {
        let available_templates = self
            .paths
            .iter()
            .map(|path| {
                let template_path = Path::new(path).join("*.yaml");
                let pattern_string = template_path.to_str().ok_or_else(|| {
                    CircuitTemplateError::new(&String::from("Template path is not valid UTF-8"))
                })?;
                Ok(glob(pattern_string).map_err(|_| {
                    CircuitTemplateError::new(&format!(
                        "Cannot query path {:?} for pattern: {}",
                        path,
                        template_path.display()
                    ))
                })?)
            })
            .collect::<Result<Vec<_>, CircuitTemplateError>>()?
            .into_iter()
            .flatten()
            // Filter out any files that can't be read
            .filter_map(|entry| match entry {
                Ok(entry) => entry.file_stem()?.to_str().map(ToOwned::to_owned),
                Err(_) => {
                    error!("Unable to read file: {:?}", entry);
                    None
                }
            })
            .collect::<Vec<String>>();

        Ok(available_templates)
    }
}

/// Searches through a list of paths to find the specified template.
pub(in crate::circuit::template) fn find_template(
    name: &str,
    paths: &[String],
) -> Result<String, CircuitTemplateError> {
    // Check if the name of the template passed in is an absolute or relative path, in which
    // case the `name` will be used as the path to the template file. If `name` is used as the path,
    // the function will not attempt to search through the possible directories to locate the
    // template file.
    let path_name = Path::new(name);
    if path_name.is_absolute() || path_name.starts_with("./") || path_name.starts_with("../") {
        return Ok(name.to_string());
    }
    // Format the template file name, to allow for the template file name to be passed in with and
    // without the file extension. For example, both "template" and "template.yaml" passed in as
    // `name` will locate the "template.yaml" template file.
    let name = format!("{}.yaml", name.trim_end_matches(".yaml"));
    // If the `paths` list is empty, populate this list with the `SPLINTER_CIRCUIT_TEMPLATE_PATH`
    // value, if set, and the default circuit template directory.
    let paths = if paths.is_empty() {
        if let Ok(template_paths) = env::var(SPLINTER_CIRCUIT_TEMPLATE_PATH) {
            paths.to_vec().extend(
                template_paths
                    .split(':')
                    .map(ToOwned::to_owned)
                    .collect::<Vec<String>>(),
            );
        }
        paths.to_vec().push(DEFAULT_TEMPLATE_DIR.to_string());
        paths
    } else {
        paths
    };
    // Check for the valid paths to the specified, using `name`, template file.
    let valid_paths: Vec<String> = paths
        .iter()
        .filter_map(|path| {
            let file_path = Path::new(path).join(&name);
            if file_path.exists() {
                file_path
                    .to_str()
                    .ok_or_else(|| {
                        CircuitTemplateError::new(&format!(
                            "Unable to find {} template file in paths: {:?}",
                            name, paths
                        ))
                    })
                    .ok()
                    .map(ToOwned::to_owned)
            } else {
                None
            }
        })
        .collect();
    // If no valid paths to the specified template file are found, an error is returned.
    if valid_paths.is_empty() {
        Err(CircuitTemplateError::new(&format!(
            "Unable to find {} template file in paths: {:?}",
            name, paths
        )))
    } else {
        // Takes the first template file path in the list, as this adheres to the precedence of the
        // of list of paths.
        Ok(valid_paths
            .first()
            .ok_or_else(|| {
                CircuitTemplateError::new(&format!(
                    "Unable to find {} template file in paths: {:?}",
                    name, paths
                ))
            })?
            .to_string())
    }
}

/// Generates a `CreateCircuitBuilder` from a circuit template file.
///
/// The circuit template outlines all required information to generate a `CreateCircuitBuilder`.
/// The required `arguments`, set by the circuit template, are used in conjunction with the template
/// `rules` to create the `CreateCircuitBuilder`.
pub struct CircuitCreateTemplate {
    version: String,
    /// Necessary arguments to build a `CreateCircuitBuilder` from the `CircuitCreateTemplate`.
    arguments: Vec<RuleArgument>,
    /// Automated process to define more complex entries of the `CreateCircuitBuilder`.
    rules: Rules,
}

impl CircuitCreateTemplate {
    /// Constructs a `CircuitCreateTemplate` from the specified YAML file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the circuit template file.
    pub fn from_yaml_file(path: &str) -> Result<Self, CircuitTemplateError> {
        let circuit_template = CircuitTemplate::load_from_file(path)?;
        match circuit_template {
            CircuitTemplate::V1(template) => Ok(Self::try_from(template)?),
        }
    }

    /// Updates a `CreateCircuitBuilder` based on the template argument values.
    ///
    /// Applies all `rules` from the circuit template using the data saved in the `arguments` to
    /// a `CreateCircuitBuilder`. Also adds services created from the circuit template to the
    /// returned builder if the `create_services` rule is in the template.
    pub fn apply_to_builder(
        &self,
        circuit_builder: CreateCircuitBuilder,
    ) -> Result<CreateCircuitBuilder, CircuitTemplateError> {
        let circuit_builder = self.rules.apply_rules(circuit_builder, &self.arguments)?;
        Ok(circuit_builder)
    }

    /// Set a required argument for a specific circuit template.
    ///
    /// # Arguments
    ///
    /// * `key` - Name of the argument to be set.
    /// * `value` - Value of the argument to be set.
    pub fn set_argument_value(
        &mut self,
        key: &str,
        value: &str,
    ) -> Result<(), CircuitTemplateError> {
        let name = key.to_lowercase();
        let (index, mut arg) = self
            .arguments
            .iter()
            .enumerate()
            .find_map(|(index, arg)| {
                if arg.name() == name {
                    Some((index, arg.clone()))
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                CircuitTemplateError::new(&format!(
                    "Argument {} is not defined in the template",
                    key
                ))
            })?;
        arg.set_user_value(value);
        self.arguments[index] = arg;
        Ok(())
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn arguments(&self) -> &[RuleArgument] {
        &self.arguments
    }

    pub fn rules(&self) -> &Rules {
        &self.rules
    }
}

impl TryFrom<v1::CircuitCreateTemplate> for CircuitCreateTemplate {
    type Error = CircuitTemplateError;
    fn try_from(create_circuit_template: v1::CircuitCreateTemplate) -> Result<Self, Self::Error> {
        Ok(CircuitCreateTemplate {
            version: create_circuit_template.version().to_string(),
            arguments: create_circuit_template
                .args()
                .to_owned()
                .into_iter()
                .map(RuleArgument::try_from)
                .collect::<Result<_, CircuitTemplateError>>()?,
            rules: Rules::from(create_circuit_template.rules().clone()),
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;

    use tempdir::TempDir;

    use crate::admin::messages::SplinterService;

    /// Example circuit template YAML file.
    const EXAMPLE_TEMPLATE_YAML: &[u8] = br##"version: v1
args:
    - name: ADMIN_KEYS
      required: false
      default: $(SIGNER_PUB_KEY)
    - name: NODES
      required: true
    - name: SIGNER_PUB_KEY
      required: false
    - name: GAMEROOM_NAME
      required: true
rules:
    set-management-type:
        management-type: "gameroom"
    create-services:
        service-type: 'scabbard'
        service-args:
        - key: 'admin-keys'
          value: [$(ADMIN_KEYS)]
        - key: 'peer_services'
          value: '$(ALL_OTHER_SERVICES)'
        first-service: 'a000'
    set-metadata:
        encoding: json
        metadata:
            - key: "scabbard_admin_keys"
              value: ["$(ADMIN_KEYS)"]
            - key: "alias"
              value: "$(GAMEROOM_NAME)" "##;

    /// Verifies the builder can be parsed from template v1 and has the correctly applied
    /// `set-management-type`, `create-services` and `set-metadata` `rules`.
    ///
    /// The test follows the procedure below:
    /// 1. Sets up a temporary directory, to write a circuit template YAML file from the
    ///    `EXAMPLE_TEMPLATE_YAML`.
    /// 2. After building a `CircuitCreateTemplate` from the circuit template YAML file, the required
    ///    `arguments` are set. These `arguments` are specific to the circuit template YAML file.
    /// 3. Apply the `CircuitCreateTemplate` to a `CreateCircuitBuilder`.
    ///
    /// Once the `CreateCircuitBuilder` object has been created, the values are asserted against
    /// the expected values. This verifies the `CircuitCreateTemplate` `rules` have been used
    /// applied successfully to the `arguments`.
    #[test]
    fn test_builds_template_v1() {
        let temp_dir = TempDir::new("test_builds_template_v1").unwrap();
        let temp_dir = temp_dir.path().to_path_buf();
        let file_path = get_file_path(temp_dir);

        write_yaml_file(&file_path, EXAMPLE_TEMPLATE_YAML);
        let mut template =
            CircuitCreateTemplate::from_yaml_file(&file_path).expect("failed to parse template");

        template
            .set_argument_value("nodes", "alpha-node-000,beta-node-000")
            .expect("Error setting argument");
        template
            .set_argument_value("signer_pub_key", "signer_key")
            .expect("Error setting argument");
        template
            .set_argument_value("gameroom_name", "my gameroom")
            .expect("Error setting argument");

        let circuit_create_builder = template
            .apply_to_builder(CreateCircuitBuilder::new())
            .expect("Error getting builders from templates");

        assert_eq!(
            circuit_create_builder.circuit_management_type(),
            Some("gameroom".to_string())
        );

        let metadata = String::from_utf8(
            circuit_create_builder
                .application_metadata()
                .expect("Application metadata is not set"),
        )
        .expect("Failed to parse metadata to string");
        assert_eq!(
            metadata,
            "{\"scabbard_admin_keys\":[\"signer_key\"],\"alias\":\"my gameroom\"}"
        );

        let service_builders: Vec<SplinterService> = circuit_create_builder
            .roster()
            .ok_or(0)
            .expect("Unable to get roster");
        let service_alpha_node = service_builders
            .iter()
            .find(|service| service.allowed_nodes == vec!["alpha-node-000".to_string()])
            .expect("service builder for alpha-node was not created correctly");

        assert_eq!(service_alpha_node.service_id, "a000".to_string());
        assert_eq!(service_alpha_node.service_type, "scabbard".to_string());

        let alpha_service_args = &service_alpha_node.arguments;
        assert!(alpha_service_args
            .iter()
            .any(|(key, value)| key == "admin-keys" && value == "[\"signer_key\"]"));
        assert!(alpha_service_args
            .iter()
            .any(|(key, value)| key == "peer_services" && value == "[\"a001\"]"));

        let service_beta_node = service_builders
            .iter()
            .find(|service| service.allowed_nodes == vec!["beta-node-000".to_string()])
            .expect("service builder for beta-node was not created correctly");

        assert_eq!(service_beta_node.service_id, "a001".to_string());
        assert_eq!(service_beta_node.service_type, "scabbard".to_string());

        let beta_service_args = &service_beta_node.arguments;
        assert!(beta_service_args
            .iter()
            .any(|(key, value)| key == "admin-keys" && value == "[\"signer_key\"]"));
        assert!(beta_service_args
            .iter()
            .any(|(key, value)| key == "peer_services" && value == "[\"a000\"]"));
    }

    /// Verifies a `CircuitTemplateManager` can be created using multiple paths, which will be
    /// be accurately used to locate the circuit template example file.
    ///
    /// The test follows the procedure below:
    /// 1. Sets up a temporary directory, to write a circuit template YAML file from the
    ///    `EXAMPLE_TEMPLATE_YAML`.
    /// 2. Create a `CircuitTemplateManager` using multiple temporary directories.
    /// 3. Write and save two example template files to separate directories used to create the
    ///    `CircuitTemplateManager` in the previous step.
    /// 4. Load a circuit template by passing in just the file name and using the
    ///    `CircuitTemplateManager`, meaning each directory must be searched to find the file.
    /// 5. Assert the template file was successfully loaded.
    /// 6. Load a second circuit template by passing in just the file name and using the
    ///    `CircuitTemplateManager`, meaning each directory must be searched to find the file.
    /// 7. Assert the second template file was successfully loaded.
    ///
    /// Asserting that each template file is successfully loaded from different directories used to
    /// create the `CircuitTemplateManager` verifies that multiple directories can be parsed to
    /// find the correct template file.
    #[test]
    fn test_multiple_template_paths() {
        let temp_dir1 = TempDir::new("test1").unwrap();
        let temp_dir1 = temp_dir1.path().to_path_buf();
        let temp_dir2 = TempDir::new("test2").unwrap();
        let temp_dir2 = temp_dir2.path().to_path_buf();

        let manager = CircuitTemplateManager::new(&[
            temp_dir1
                .to_str()
                .expect("Unable to create str from temp_dir1 path")
                .to_string(),
            temp_dir2
                .to_str()
                .expect("Unable to create str from temp_dir2 path")
                .to_string(),
        ]);

        let file_path1 = get_file_path(temp_dir1);
        write_yaml_file(&file_path1, EXAMPLE_TEMPLATE_YAML);

        let template = manager.load("example_template.yaml");
        assert!(template.is_ok());

        let mut file_path2 = temp_dir2;
        file_path2.push("example_template2.yaml");
        let file_path2 = file_path2.to_str().unwrap().to_string();
        write_yaml_file(&file_path2, EXAMPLE_TEMPLATE_YAML);

        let template = manager.load("example_template2.yaml");
        assert!(template.is_ok());
    }

    fn get_file_path(mut temp_dir: PathBuf) -> String {
        temp_dir.push("example_template.yaml");
        let path = temp_dir.to_str().unwrap().to_string();
        path
    }

    fn write_yaml_file(file_path: &str, data: &[u8]) {
        let mut file = File::create(file_path).expect("Error creating test template yaml file.");

        file.write_all(data)
            .expect("Error writing example template yaml.");
    }
}
