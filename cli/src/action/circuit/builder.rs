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

#[cfg(feature = "circuit-auth-type")]
use splinter::admin::messages::AuthorizationType;
use splinter::admin::messages::{
    BuilderError, CreateCircuit, CreateCircuitBuilder, SplinterNode, SplinterNodeBuilder,
    SplinterServiceBuilder,
};

use crate::error::CliError;
use crate::store::default_value::DefaultValueStore;

use super::defaults::{get_default_value_store, MANAGEMENT_TYPE_KEY, SERVICE_TYPE_KEY};

const PEER_SERVICES_ARG: &str = "peer_services";

pub struct CreateCircuitMessageBuilder {
    create_circuit_builder: CreateCircuitBuilder,
    services: Vec<SplinterServiceBuilder>,
    nodes: Vec<SplinterNode>,
    management_type: Option<String>,
    #[cfg(feature = "circuit-auth-type")]
    authorization_type: Option<AuthorizationType>,
    application_metadata: Vec<u8>,
    comments: Option<String>,
}

impl CreateCircuitMessageBuilder {
    pub fn new() -> CreateCircuitMessageBuilder {
        CreateCircuitMessageBuilder {
            create_circuit_builder: CreateCircuitBuilder::new(),
            services: vec![],
            nodes: vec![],
            management_type: None,
            #[cfg(feature = "circuit-auth-type")]
            authorization_type: None,
            application_metadata: vec![],
            comments: None,
        }
    }

    #[cfg(feature = "circuit-template")]
    pub fn add_services(&mut self, service_builders: &[SplinterServiceBuilder]) {
        self.services.extend(service_builders.to_owned());
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

    pub fn add_node(&mut self, node_id: &str, node_endpoint: &str) -> Result<(), CliError> {
        for node in &self.nodes {
            if node.node_id == node_id {
                return Err(CliError::ActionError(format!(
                    "Duplicate node ID detected: {}",
                    node_id
                )));
            }
            if node.endpoint == node_endpoint {
                return Err(CliError::ActionError(format!(
                    "Duplicate node endpoint detected: {}",
                    node_endpoint
                )));
            }
        }

        self.nodes.push(make_splinter_node(node_id, node_endpoint)?);
        Ok(())
    }

    pub fn set_management_type(&mut self, management_type: &str) {
        self.management_type = Some(management_type.into());
    }

    #[cfg(feature = "circuit-auth-type")]
    pub fn set_authorization_type(&mut self, authorization_type: &str) -> Result<(), CliError> {
        let auth_type = match authorization_type {
            "trust" => AuthorizationType::Trust,
            _ => {
                return Err(CliError::ActionError(format!(
                    "Invalid authorization type {}",
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

    pub fn build(self) -> Result<CreateCircuit, CliError> {
        let default_store = get_default_value_store();

        // if management type is not set check for default value
        let management_type =
            match self.management_type {
                Some(management_type) => management_type,
                None => match self.create_circuit_builder.circuit_management_type() {
                    Some(management_type) => management_type,
                    None => match default_store.get_default_value(MANAGEMENT_TYPE_KEY)? {
                        Some(management_type) => management_type.value(),
                        None => return Err(CliError::ActionError(
                            "Failed to build circuit: Management type not provided and no default \
                             set"
                            .into(),
                        )),
                    },
                },
            };

        let services = self
            .services
            .into_iter()
            .map(|mut builder| {
                let service_id = builder.service_id().unwrap_or_default();
                // if service type is not set, check for default value
                if builder.service_type().is_none() {
                    builder = match default_store.get_default_value(SERVICE_TYPE_KEY)? {
                        Some(service_type) => builder.with_service_type(&service_type.value()),
                        None => {
                            return Err(CliError::ActionError(format!(
                                "Failed to build service '{}': Service type not provided and no \
                                 default set",
                                service_id,
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

        let create_circuit_builder = self
            .create_circuit_builder
            .with_members(&self.nodes)
            .with_roster(&services)
            .with_circuit_management_type(&management_type)
            .with_application_metadata(&self.application_metadata)
            .with_comments(&self.comments.unwrap_or_default());

        #[cfg(feature = "circuit-auth-type")]
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

fn make_splinter_node(node_id: &str, endpoint: &str) -> Result<SplinterNode, CliError> {
    let node = SplinterNodeBuilder::new()
        .with_node_id(&node_id)
        .with_endpoint(&endpoint)
        .build()
        .map_err(|err| {
            CliError::ActionError(format!(
                "Failed to build node: {}",
                msg_from_builder_error(err)
            ))
        })?;
    Ok(node)
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
