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

//! Contains the implementation of `Network`.

use std::collections::HashMap;
use std::fs::File;

use cylinder::{secp256k1::Secp256k1Context, Context, Signer};
use splinter::error::{InternalError, InvalidArgumentError};
use splinter::registry::Node as RegistryNode;
use splinter::threading::lifecycle::ShutdownHandle;
use splinterd::node::{
    Node, NodeBuilder, PermissionConfig, RestApiVariant, RunnableNode, ScabbardConfigBuilder,
};
use tempdir::TempDir;

use super::circuit_builder::CircuitBuilder;

pub struct Network {
    default_rest_api_variant: RestApiVariant,
    nodes: Vec<NetworkNode>,
    temp_dirs: HashMap<String, TempDir>,
    external_registries: Option<Vec<String>>,
    num_of_keys: usize,
    cylinder_auth: bool,
    permission_config: Option<Vec<PermissionConfig>>,
    admin_signer: Option<Box<dyn Signer>>,
}

pub enum NetworkNode {
    Node(Node),
    RunnableNode(RunnableNode),
}

impl Network {
    pub fn new() -> Network {
        Network {
            default_rest_api_variant: RestApiVariant::ActixWeb1,
            nodes: Vec::new(),
            temp_dirs: HashMap::new(),
            external_registries: None,
            num_of_keys: 1,
            cylinder_auth: true,
            permission_config: None,
            admin_signer: None,
        }
    }

    pub fn with_cylinder_auth(mut self) -> Self {
        self.cylinder_auth = true;
        self
    }

    pub fn add_nodes_with_defaults(mut self, count: i32) -> Result<Network, InternalError> {
        let mut registry_info = vec![];
        let context = Secp256k1Context::new();
        for _ in 0..count {
            let admin_signer = match self.admin_signer {
                Some(ref signer) => signer.clone_box(),
                None => context.new_signer(context.new_random_private_key()),
            };
            let public_key = admin_signer
                .public_key()
                .map_err(|e| InternalError::from_source(Box::new(e)))?;
            let temp_dir = TempDir::new("scabbard_data")
                .map_err(|e| InternalError::from_source(Box::new(e)))?;
            let temp_db_path = temp_dir.path().join("sqlite_receipt_store.db");

            File::create(temp_db_path.clone())
                .map_err(|e| InternalError::from_source(Box::new(e)))?;

            let mut signers = Vec::new();
            for _ in 0..self.num_of_keys {
                signers.push(context.new_signer(context.new_random_private_key()));
            }

            let mut builder = NodeBuilder::new()
                .with_rest_api_variant(self.default_rest_api_variant)
                .with_scabbard(
                    ScabbardConfigBuilder::new()
                        .with_data_dir(temp_dir.path().to_path_buf())
                        .with_receipt_db_url(
                            temp_db_path
                                .to_str()
                                .ok_or_else(|| {
                                    InternalError::with_message(
                                        "failed to convert db path to str".to_string(),
                                    )
                                })?
                                .to_string(),
                        )
                        .build()?,
                )
                .with_admin_signer(admin_signer)
                .with_signers(signers)
                .with_external_registries(self.external_registries.clone())
                .with_biome_enabled()
                .with_permission_config(self.permission_config.clone());
            if self.cylinder_auth {
                builder = builder.with_cylinder_auth(Box::new(Secp256k1Context::new()));
            }

            let node = builder.build()?.run()?;

            registry_info.push((
                node.node_id().to_string(),
                public_key,
                node.network_endpoints().to_vec(),
            ));

            self.temp_dirs.insert(node.node_id().to_string(), temp_dir);
            self.nodes.push(NetworkNode::Node(node));
        }

        for node in &self.nodes {
            match node {
                NetworkNode::Node(node) => {
                    let registry_writer = node.registry_writer();
                    for (node_id, pub_key, endpoints) in &registry_info {
                        registry_writer
                            .add_node(
                                RegistryNode::builder(node_id)
                                    .with_display_name(node_id)
                                    .with_endpoints(endpoints.to_vec())
                                    .with_key(pub_key.as_hex())
                                    .build()
                                    .map_err(|e| InternalError::from_source(Box::new(e)))?,
                            )
                            .map_err(|e| InternalError::from_source(Box::new(e)))?;
                    }
                }
                _ => unreachable!(), // a new network will only contain running nodes
            }
        }

        Ok(self)
    }

    pub fn with_default_rest_api_variant(mut self, variant: RestApiVariant) -> Self {
        self.default_rest_api_variant = variant;
        self
    }

    pub fn set_num_of_keys(mut self, num_of_keys: usize) -> Self {
        self.num_of_keys = num_of_keys;
        self
    }

    pub fn with_permission_config(mut self, permission_config: Vec<PermissionConfig>) -> Self {
        self.permission_config = Some(permission_config);
        self
    }

    pub fn with_admin_signer(mut self, signer: Box<dyn Signer>) -> Self {
        self.admin_signer = Some(signer);
        self
    }

    pub fn node(&self, n: usize) -> Result<&Node, InvalidArgumentError> {
        match self.nodes.get(n) {
            Some(network_node) => match network_node {
                NetworkNode::Node(node) => Ok(node),
                NetworkNode::RunnableNode(_) => Err(InvalidArgumentError::new(
                    "n".to_string(),
                    "node is stopped".to_string(),
                )),
            },
            None => Err(InvalidArgumentError::new(
                "n".to_string(),
                "out of range".to_string(),
            )),
        }
    }

    /// Create a [`CircuitBuilder`] with the given the node indices
    pub fn circuit_builder<'a>(
        &'a self,
        nodes: &[usize],
    ) -> Result<CircuitBuilder<'a>, InvalidArgumentError> {
        CircuitBuilder::new(self, nodes)
    }

    pub fn start(mut self, index: usize) -> Result<Network, InternalError> {
        let node = match self.nodes.remove(index) {
            NetworkNode::RunnableNode(runnable_node) => runnable_node.run()?,
            NetworkNode::Node(_) => {
                return Err(InternalError::with_message(
                    "node is already running".to_string(),
                ))
            }
        };

        let registry_writer = node.registry_writer();

        // Update the registry
        registry_writer
            .update_node(
                RegistryNode::builder(node.node_id().to_string())
                    .with_display_name(node.node_id().to_string())
                    .with_endpoints(node.network_endpoints().to_vec())
                    .with_key(
                        node.admin_signer()
                            .clone_box()
                            .public_key()
                            .map_err(|e| InternalError::from_source(Box::new(e)))?
                            .as_hex(),
                    )
                    .build()
                    .map_err(|e| InternalError::from_source(Box::new(e)))?,
            )
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        self.nodes.insert(index, NetworkNode::Node(node));

        Ok(self)
    }

    pub fn stop(mut self, index: usize) -> Result<Network, InternalError> {
        let runnable_node = match self.nodes.remove(index) {
            NetworkNode::Node(node) => node.stop()?,
            NetworkNode::RunnableNode(_) => {
                return Err(InternalError::with_message(
                    "node is already stopped".to_string(),
                ))
            }
        };
        self.nodes
            .insert(index, NetworkNode::RunnableNode(runnable_node));

        Ok(self)
    }
}

impl ShutdownHandle for Network {
    fn signal_shutdown(&mut self) {
        for node in &mut self.nodes {
            match node {
                NetworkNode::Node(node) => node.signal_shutdown(),
                NetworkNode::RunnableNode(_) => (),
            }
        }
    }

    fn wait_for_shutdown(self) -> Result<(), InternalError> {
        for node in self.nodes.into_iter() {
            match node {
                NetworkNode::Node(node) => node.wait_for_shutdown()?,
                NetworkNode::RunnableNode(_) => (),
            }
        }

        Ok(())
    }
}

impl Default for Network {
    fn default() -> Self {
        Self::new()
    }
}
