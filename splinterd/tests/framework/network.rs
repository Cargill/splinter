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

//! Contains the implementation of `Network`.

use splinter::error::{InternalError, InvalidArgumentError};
use splinter::threading::shutdown::ShutdownHandle;
use splinterd::node::{Node, NodeBuilder, RestApiVariant};

pub struct Network {
    default_rest_api_variant: RestApiVariant,
    nodes: Vec<Node>,
}

impl Network {
    pub fn new() -> Network {
        Network {
            default_rest_api_variant: RestApiVariant::ActixWeb1,
            nodes: Vec::new(),
        }
    }

    pub fn add_nodes_with_defaults(mut self, count: i32) -> Result<Network, InternalError> {
        for _ in 0..count {
            let node = NodeBuilder::new()
                .with_rest_api_variant(self.default_rest_api_variant)
                .build()?
                .run()?;
            self.nodes.push(node);
        }

        Ok(self)
    }

    pub fn with_default_rest_api_variant(mut self, variant: RestApiVariant) -> Self {
        self.default_rest_api_variant = variant;
        self
    }

    pub fn node(self: &mut Network, n: usize) -> Result<&mut Node, InvalidArgumentError> {
        match self.nodes.get_mut(n) {
            Some(node) => Ok(node),
            None => Err(InvalidArgumentError::new(
                "n".to_string(),
                "out of range".to_string(),
            )),
        }
    }
}

impl ShutdownHandle for Network {
    fn signal_shutdown(&mut self) {
        for node in &mut self.nodes {
            node.signal_shutdown()
        }
    }

    fn wait_for_shutdown(self: Box<Self>) -> Result<(), InternalError> {
        for node in self.nodes.into_iter() {
            Box::new(node).wait_for_shutdown()?;
        }

        Ok(())
    }
}

impl Default for Network {
    fn default() -> Self {
        Self::new()
    }
}
