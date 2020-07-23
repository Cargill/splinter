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

//! Defines the `CircuitCreateTemplate` based on the current version of circuits, version 1.

/// Struct to hold the necessary `rules` and `args` required to create a `CreateCircuitBuilder`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CircuitCreateTemplate {
    /// Version of the circuit definition.
    version: String,
    /// Required data to fill out the circuit template.
    args: Vec<RuleArgument>,
    /// Automated process to define more complex entries of the `CreateCircuitBuilder`.
    rules: Rules,
}

impl CircuitCreateTemplate {
    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn args(&self) -> &[RuleArgument] {
        &self.args
    }

    pub fn rules(&self) -> &Rules {
        &self.rules
    }
}

/// Struct to hold the value of and information about an argument.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RuleArgument {
    /// Name of the argument.
    name: String,
    /// Whether or not the argument is required for the `CircuitCreateTemplate`.
    required: bool,
    /// Optional value of the argument.
    #[serde(rename = "default")]
    #[serde(skip_serializing_if = "Option::is_none")]
    default_value: Option<String>,
    /// Optional description of the argument.
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

impl RuleArgument {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn required(&self) -> bool {
        self.required
    }

    pub fn default_value(&self) -> Option<&String> {
        self.default_value.as_ref()
    }

    pub fn description(&self) -> Option<&String> {
        self.description.as_ref()
    }
}

/// Struct to hold the defined `rules`, which are automated processes to define entries of the
/// `CreateCircuitBuilder` based on the `args` values.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Rules {
    /// Process for defining the `circuit_management_type` of a circuit.
    #[serde(skip_serializing_if = "Option::is_none")]
    set_management_type: Option<CircuitManagement>,
    /// Process for defining the services of a circuit.
    #[serde(skip_serializing_if = "Option::is_none")]
    create_services: Option<CreateServices>,
    /// Process for defining the `metadata` field of a circuit.
    #[serde(skip_serializing_if = "Option::is_none")]
    set_metadata: Option<SetMetadata>,
}

impl Rules {
    pub fn set_management_type(&self) -> Option<&CircuitManagement> {
        self.set_management_type.as_ref()
    }

    pub fn create_services(&self) -> Option<&CreateServices> {
        self.create_services.as_ref()
    }

    pub fn set_metadata(&self) -> Option<&SetMetadata> {
        self.set_metadata.as_ref()
    }
}

/// The `management_type` used in the `set_management_type` rule.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct CircuitManagement {
    management_type: String,
}

impl CircuitManagement {
    pub fn management_type(&self) -> &str {
        &self.management_type
    }
}

/// Struct to wrap the information used to define a `SplinterService`.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct CreateServices {
    /// Type of the `SplinterService` being constructed.
    service_type: String,
    /// Arguments required to build the `SplinterService`.
    service_args: Vec<ServiceArgument>,
    first_service: String,
}

impl CreateServices {
    pub fn service_type(&self) -> &str {
        &self.service_type
    }

    pub fn service_args(&self) -> &[ServiceArgument] {
        &self.service_args
    }

    pub fn first_service(&self) -> &str {
        &self.first_service
    }
}

/// Struct to wrap the name and value for an argument of a `SplinterService`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceArgument {
    key: String,
    value: Value,
}

impl ServiceArgument {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
}

/// Struct to wrap the `metadata` used in the `set_metadata` rule.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SetMetadata {
    #[serde(flatten)]
    metadata: Metadata,
}

impl SetMetadata {
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }
}

/// Enum of the possible types of `metadata` representations.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "encoding")]
#[serde(rename_all(deserialize = "camelCase"))]
pub enum Metadata {
    Json { metadata: Vec<JsonMetadata> },
}

/// Struct of the data held in the `Metadata` object.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonMetadata {
    key: String,
    value: Value,
}

impl JsonMetadata {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
}

/// Struct to represent single and list values within the `JsonMetadata`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum Value {
    Single(String),
    List(Vec<String>),
}
