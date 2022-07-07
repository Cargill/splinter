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

use std::fmt::Write as _;

use splinter::admin::messages::AuthorizationType;
use splinter::admin::messages::{
    BuilderError, CircuitStatus, CreateCircuit, CreateCircuitBuilder, SplinterNode,
    SplinterNodeBuilder, SplinterServiceBuilder,
};

use crate::error::CliError;

const PEER_SERVICES_ARG: &str = "peer_services";
const MANAGEMENT_TYPE_ENV: &str = "SPLINTER_CIRCUIT_MANAGEMENT_TYPE";
const SERVICE_TYPE_ENV: &str = "SPLINTER_CIRCUIT_SERVICE_TYPE";

pub struct CreateCircuitMessageBuilder {
    create_circuit_builder: CreateCircuitBuilder,
    services: Vec<SplinterServiceBuilder>,
    nodes: Vec<SplinterNode>,
    management_type: Option<String>,
    authorization_type: Option<AuthorizationType>,
    application_metadata: Vec<u8>,
    comments: Option<String>,
    display_name: Option<String>,
    circuit_version: Option<i32>,
    circuit_status: Option<CircuitStatus>,
}

impl CreateCircuitMessageBuilder {
    pub fn new() -> CreateCircuitMessageBuilder {
        CreateCircuitMessageBuilder {
            create_circuit_builder: CreateCircuitBuilder::new(),
            services: vec![],
            nodes: vec![],
            management_type: None,
            authorization_type: None,
            application_metadata: vec![],
            comments: None,
            display_name: None,
            circuit_version: None,
            circuit_status: None,
        }
    }

    pub fn create_circuit_builder(&self) -> CreateCircuitBuilder {
        self.create_circuit_builder.clone()
    }

    #[cfg(feature = "circuit-template")]
    pub fn set_create_circuit_builder(&mut self, create_circuit_builder: &CreateCircuitBuilder) {
        self.create_circuit_builder = create_circuit_builder.clone()
    }

    #[cfg(feature = "circuit-template")]
    pub fn get_node_ids(&self) -> Vec<String> {
        self.nodes.iter().map(|node| node.node_id.clone()).collect()
    }

    pub fn apply_service_type(&mut self, service_id_match: &str, service_type: &str) {
        // Clone the service builders, add the type to matching services builders, and use the
        // updated builders to replace the existing ones.
        self.services = self
            .services
            .clone()
            .into_iter()
            .map(|service_builder| {
                let service_id = service_builder.service_id().unwrap_or_default();
                if is_match(service_id_match, &service_id) {
                    service_builder.with_service_type(service_type)
                } else {
                    service_builder
                }
            })
            .collect();
    }

    pub fn apply_service_arguments(
        &mut self,
        service_id_match: &str,
        arg: &(String, String),
    ) -> Result<(), CliError> {
        // Clone the service builders, add the argument to matching services builders, and use the
        // updated builders to replace the existing ones.
        self.services = self
            .services
            .clone()
            .into_iter()
            .map(|service_builder| {
                // Determine if the service builder matches the pattern
                let service_id = service_builder.service_id().unwrap_or_default();
                if is_match(service_id_match, &service_id) {
                    let mut service_args = service_builder.arguments().unwrap_or_default();

                    // Check for duplicate argument
                    let key = &arg.0;
                    if service_args.iter().any(|arg| &arg.0 == key) {
                        return Err(CliError::ActionError(format!(
                            "Duplicate service argument '{}' detected for service '{}'",
                            key, service_id,
                        )));
                    }

                    // Add the argument
                    service_args.push(arg.clone());
                    Ok(service_builder.with_arguments(&service_args))
                } else {
                    // Pattern didn't match, so leave the builder as-is
                    Ok(service_builder)
                }
            })
            .collect::<Result<_, _>>()?;
        Ok(())
    }

    pub fn apply_peer_services(&mut self, service_id_globs: &[&str]) -> Result<(), CliError> {
        // Get list of all peer IDs that are matched by the service ID globs
        let peers = self
            .services
            .iter()
            .filter_map(|service_builder| {
                let service_id = service_builder.service_id().unwrap_or_default();
                if service_id_globs
                    .iter()
                    .any(|glob| is_match(glob, &service_id))
                {
                    Some(service_id)
                } else {
                    None
                }
            })
            .collect::<Vec<String>>();

        // Clone the service builders, add PEER_SERVICES_ARG to matching services builders, and use
        // the updated builders to replace the existing ones.
        self.services = self
            .services
            .clone()
            .into_iter()
            .map(|service_builder| {
                // Determine if the builder is in the list of IDs and get the index of its ID
                let service_id = service_builder.service_id().unwrap_or_default();
                let index = peers.iter().position(|peer_id| peer_id == &service_id);

                if let Some(index) = index {
                    // Copy the list of IDs and remove the builder's own ID, since it won't be a
                    // peer of itself
                    let mut service_peers = peers.clone();
                    service_peers.remove(index);

                    // Check if the argument has already been set
                    let mut service_args = service_builder.arguments().unwrap_or_default();
                    if service_args.iter().any(|arg| arg.0 == PEER_SERVICES_ARG) {
                        return Err(CliError::ActionError(format!(
                            "Peer services for service '{}' is already set",
                            service_id,
                        )));
                    }

                    // Add the argument
                    service_args.push((
                        PEER_SERVICES_ARG.into(),
                        format!("[\"{}\"]", service_peers.join("\", \"")),
                    ));
                    Ok(service_builder.with_arguments(&service_args))
                } else {
                    // Pattern didn't match, so leave the builder as-is
                    Ok(service_builder)
                }
            })
            .collect::<Result<_, _>>()?;

        Ok(())
    }

    pub fn add_node(
        &mut self,
        node_id: &str,
        node_endpoints: &[String],
        public_key: Option<&String>,
    ) -> Result<(), CliError> {
        for node in &self.nodes {
            if node.node_id == node_id {
                return Err(CliError::ActionError(format!(
                    "Duplicate node ID detected: {}",
                    node_id
                )));
            }
            if let Some(endpoint) = node_endpoints
                .iter()
                .find(|endpoint| node.endpoints.contains(endpoint))
            {
                return Err(CliError::ActionError(format!(
                    "Duplicate node endpoint detected: {}",
                    endpoint
                )));
            }
        }

        self.nodes
            .push(make_splinter_node(node_id, node_endpoints, public_key)?);

        Ok(())
    }

    pub fn set_management_type(&mut self, management_type: &str) {
        self.management_type = Some(management_type.into());
    }

    pub fn set_authorization_type(&mut self, authorization_type: &str) -> Result<(), CliError> {
        let auth_type = match authorization_type {
            "trust" => AuthorizationType::Trust,
            "challenge" => AuthorizationType::Challenge,
            _ => {
                return Err(CliError::ActionError(format!(
                    "Invalid authorization type: {}",
                    authorization_type
                )))
            }
        };

        self.authorization_type = Some(auth_type);
        Ok(())
    }

    pub fn set_application_metadata(&mut self, application_metadata: &[u8]) {
        self.application_metadata = application_metadata.into();
    }

    pub fn set_comments(&mut self, comments: &str) {
        self.comments = Some(comments.into());
    }

    pub fn set_display_name(&mut self, display_name: &str) {
        self.display_name = Some(display_name.into());
    }

    pub fn set_circuit_version(&mut self, circuit_version: i32) {
        self.circuit_version = Some(circuit_version);
    }

    pub fn set_circuit_status(&mut self, circuit_status: CircuitStatus) {
        self.circuit_status = Some(circuit_status);
    }

    pub fn build(mut self) -> Result<CreateCircuit, CliError> {
        let circuit_builder = self.create_circuit_builder();

        // if management type is not set, check for environment variable
        let management_type = self
            .management_type
            .or_else(|| std::env::var(MANAGEMENT_TYPE_ENV).ok())
            .or_else(|| circuit_builder.circuit_management_type())
            .ok_or_else(|| {
                CliError::ActionError(
                    "Failed to build circuit: Management type not provided".into(),
                )
            })?;

        let mut services = self
            .services
            .into_iter()
            .map(|mut builder| {
                let service_id = builder.service_id().unwrap_or_default();
                // if service type is not set, check for environment variable
                if builder.service_type().is_none() {
                    match std::env::var(SERVICE_TYPE_ENV) {
                        Ok(service_type) => builder = builder.with_service_type(&service_type),
                        Err(_) => {
                            return Err(CliError::ActionError(format!(
                                "Failed to build service '{}': Service type not provided",
                                service_id
                            )))
                        }
                    }
                }

                builder.build().map_err(|err| {
                    CliError::ActionError(format!(
                        "Failed to build service '{}': {}",
                        service_id,
                        msg_from_builder_error(err)
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        if let Some(builder_roster) = circuit_builder.roster() {
            services.extend(builder_roster);
        }

        if let Some(builder_metadata) = circuit_builder.application_metadata() {
            self.application_metadata.extend(builder_metadata);
        }

        let mut comments = self.comments.unwrap_or_default();
        if let Some(builder_comments) = circuit_builder.comments() {
            write!(comments, "; {}", builder_comments)
                .map_err(|e| CliError::ActionError(e.to_string()))?;
        }

        let mut create_circuit_builder = self
            .create_circuit_builder
            .with_members(&self.nodes)
            .with_roster(&services)
            .with_circuit_management_type(&management_type)
            .with_application_metadata(&self.application_metadata)
            .with_comments(&comments);

        if let Some(display_name) = self.display_name {
            create_circuit_builder = create_circuit_builder.with_display_name(&display_name);
        }

        if let Some(circuit_version) = self.circuit_version {
            create_circuit_builder = create_circuit_builder.with_circuit_version(circuit_version);
        }

        if let Some(circuit_status) = self.circuit_status {
            create_circuit_builder = create_circuit_builder.with_circuit_status(&circuit_status);
        }

        let create_circuit_builder = match self.authorization_type {
            Some(authorization_type) => {
                create_circuit_builder.with_authorization_type(&authorization_type)
            }
            None => create_circuit_builder,
        };

        let create_circuit = create_circuit_builder.build().map_err(|err| {
            CliError::ActionError(format!(
                "Failed to build circuit: {}",
                msg_from_builder_error(err)
            ))
        })?;
        Ok(create_circuit)
    }

    pub fn add_service(
        &mut self,
        service_id: &str,
        allowed_nodes: &[String],
    ) -> Result<(), CliError> {
        if self
            .services
            .iter()
            .any(|service_builder| service_builder.service_id().unwrap_or_default() == service_id)
        {
            return Err(CliError::ActionError(format!(
                "Duplicate service ID detected: {}",
                service_id,
            )));
        }

        let service_builder = SplinterServiceBuilder::new()
            .with_service_id(service_id)
            .with_allowed_nodes(allowed_nodes);
        self.services.push(service_builder);

        Ok(())
    }
}

fn is_match(service_id_match: &str, service_id: &str) -> bool {
    service_id_match.split('*').fold(true, |is_match, part| {
        if part.len() != service_id_match.len() {
            is_match && service_id.contains(part)
        } else {
            service_id == part
        }
    })
}

fn make_splinter_node(
    node_id: &str,
    endpoints: &[String],
    public_key: Option<&String>,
) -> Result<SplinterNode, CliError> {
    #[allow(unused_mut)]
    let mut node_builder = SplinterNodeBuilder::new()
        .with_node_id(node_id)
        .with_endpoints(endpoints);

    if let Some(public_key) = public_key {
        node_builder = node_builder.with_public_key(&parse_hex(public_key)?)
    }

    let node = node_builder.build().map_err(|err| {
        CliError::ActionError(format!(
            "Failed to build node: {}",
            msg_from_builder_error(err)
        ))
    })?;
    Ok(node)
}

pub fn parse_hex(hex: &str) -> Result<Vec<u8>, CliError> {
    if hex.len() % 2 != 0 {
        return Err(CliError::ActionError(format!(
            "{} is not valid hex: odd number of digits",
            hex
        )));
    }

    let mut res = vec![];
    for i in (0..hex.len()).step_by(2) {
        res.push(
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|_| CliError::ActionError(format!("{} contains invalid hex", hex)))?,
        );
    }

    Ok(res)
}

fn msg_from_builder_error(err: BuilderError) -> String {
    match err {
        BuilderError::InvalidField(msg) => msg,
        BuilderError::MissingField(field) => format!("Missing node parameter: {}", field),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that the `is_match` method for matching (potentially wild-carded) service IDs works
    /// correctly.
    #[test]
    fn service_id_matching() {
        assert!(is_match("abcd", "abcd"));
        assert!(is_match("*bcd", "abcd"));
        assert!(is_match("*cd", "abcd"));
        assert!(is_match("*d", "abcd"));
        assert!(is_match("*", "abcd"));
        assert!(is_match("a*", "abcd"));
        assert!(is_match("ab*", "abcd"));
        assert!(is_match("abc*", "abcd"));
        assert!(is_match("a*cd", "abcd"));
        assert!(is_match("ab*d", "abcd"));
        assert!(is_match("a*d", "abcd"));
        assert!(is_match("*b*d", "abcd"));
        assert!(is_match("a*c*", "abcd"));

        assert!(!is_match("0123", "abcd"));
        assert!(!is_match("0*", "abcd"));
        assert!(!is_match("*0", "abcd"));
        assert!(!is_match("0*0", "abcd"));
        assert!(!is_match("*0*", "abcd"));
    }
}
