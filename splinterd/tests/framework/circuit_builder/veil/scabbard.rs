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

use crate::framework::circuit_builder::{
    AddScabbardServiceError, CircuitBuildError, CircuitBuilder, CircuitData, NodeCollection,
};
use crate::framework::network::Network;
use splinter::admin::messages::{SplinterService, SplinterServiceBuilder};
use splinter::error::{InternalError, InvalidArgumentError};
use splinterd::node::Node;

/// Creates a builder for a Scabbard circuit
pub struct ScabbardCircuitBuilderVeil<'a, N = Network>
where
    N: NodeCollection,
{
    circuit_builder: CircuitBuilder<'a, N>,
}

impl<'a, N> ScabbardCircuitBuilderVeil<'a, N>
where
    N: NodeCollection + 'a,
{
    /// Add a Scabbard service group to a circuit with the given nodes, where the passed nodes are
    /// the node indices from the associated network
    pub fn add_service_group(mut self, nodes: &[usize]) -> Result<Self, AddScabbardServiceError> {
        let network = self.circuit_builder.network();

        let nodes = nodes
            .iter()
            .map(|i| network.node(*i))
            .collect::<Result<Vec<&Node>, InvalidArgumentError>>()
            .map_err(AddScabbardServiceError::InvalidArgument)?;

        let keys = nodes
            .iter()
            .map(|node| {
                Ok(node
                    .admin_signer()
                    .public_key()
                    .map_err(|e| {
                        AddScabbardServiceError::Internal(InternalError::from_source(Box::new(e)))
                    })?
                    .as_hex())
            })
            .collect::<Result<Vec<String>, AddScabbardServiceError>>()?;

        let mut service_builders: Vec<(String, SplinterServiceBuilder)> = vec![];
        let mut service_ids: Vec<String> = vec![];
        for node in nodes.iter() {
            let service_id = self.circuit_builder.service_id_generator().next("sc");
            service_ids.push(service_id.clone());
            let builder = SplinterServiceBuilder::new()
                .with_service_id(service_id.as_ref())
                .with_service_type("scabbard")
                .with_allowed_nodes(vec![node.node_id().to_string()].as_ref());
            service_builders.push((service_id, builder));
        }
        let mut services: Vec<SplinterService> = service_builders
            .into_iter()
            .map(|(service_id, builder)| {
                let peer_services = service_ids
                    .iter()
                    .filter(|peer_service_id| peer_service_id != &&service_id)
                    .collect::<Vec<&String>>();
                builder
                    .with_arguments(
                        vec![
                            ("peer_services".to_string(), format!("{:?}", peer_services)),
                            (
                                "admin_keys".to_string(),
                                format!("{:?}", &keys[..].to_vec()),
                            ),
                        ]
                        .as_ref(),
                    )
                    .build()
                    .map_err(|e| {
                        AddScabbardServiceError::Internal(InternalError::from_source(Box::new(e)))
                    })
            })
            .collect::<Result<Vec<SplinterService>, AddScabbardServiceError>>()?;

        let roster = match self.circuit_builder.roster() {
            Some(mut roster) => {
                services.append(&mut roster);
                &services
            }
            None => &services,
        };
        self.circuit_builder = self.circuit_builder.with_roster(roster);
        Ok(self)
    }

    pub fn unveil(self) -> CircuitBuilder<'a, N> {
        self.circuit_builder
    }

    pub fn build(self) -> Result<CircuitData<'a>, CircuitBuildError> {
        self.circuit_builder.build()
    }
}

impl<'a, N: NodeCollection> From<CircuitBuilder<'a, N>> for ScabbardCircuitBuilderVeil<'a, N> {
    fn from(circuit_builder: CircuitBuilder<'a, N>) -> ScabbardCircuitBuilderVeil<'a, N> {
        ScabbardCircuitBuilderVeil { circuit_builder }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    enum CircuitTestError {
        Internal(InternalError),
        InvalidArgument(InvalidArgumentError),
        AddScabbardServiceError(AddScabbardServiceError),
        CircuitBuildError(CircuitBuildError),
    }

    /// Verify that the scabbard builder correctly errors and does not fatal on invalid nodes
    #[test]
    fn scabbard_builder_add_service_group_invalid_nodes() -> Result<(), CircuitTestError> {
        let network = Network::new()
            .add_nodes_with_defaults(2)
            .map_err(CircuitTestError::Internal)?;

        let builder = network
            .circuit_builder(&[0, 1])
            .map_err(CircuitTestError::InvalidArgument)?
            .veil::<ScabbardCircuitBuilderVeil>();

        if let Err(AddScabbardServiceError::InvalidArgument(_)) =
            builder.add_service_group(&[10, 20])
        {
            return Ok(());
        }

        panic!("did not receive expected error");
    }

    /// Verify scabbard builder with 4 paired service groups
    #[test]
    fn scabbard_builder_add_service_4_paired_service_groups() -> Result<(), CircuitTestError> {
        let network = Network::new()
            .add_nodes_with_defaults(4)
            .map_err(CircuitTestError::Internal)?;

        let result = network
            .circuit_builder(&[0, 1, 2, 3])
            .map_err(CircuitTestError::InvalidArgument)?
            .veil::<ScabbardCircuitBuilderVeil>()
            .add_service_group(&[0, 1])
            .map_err(CircuitTestError::AddScabbardServiceError)?
            .add_service_group(&[1, 2])
            .map_err(CircuitTestError::AddScabbardServiceError)?
            .add_service_group(&[2, 3])
            .map_err(CircuitTestError::AddScabbardServiceError)?
            .add_service_group(&[3, 0])
            .map_err(CircuitTestError::AddScabbardServiceError)?
            .build()
            .map_err(CircuitTestError::CircuitBuildError)?;

        assert_eq!(result.roster.len(), 8);

        Ok(())
    }

    /// Verify scabbard builder with 1 group of 4
    #[test]
    fn scabbard_builder_add_service_group_of_4() -> Result<(), CircuitTestError> {
        let network = Network::new()
            .add_nodes_with_defaults(4)
            .map_err(CircuitTestError::Internal)?;

        let result = network
            .circuit_builder(&[0, 1, 2, 3])
            .map_err(CircuitTestError::InvalidArgument)?
            .veil::<ScabbardCircuitBuilderVeil>()
            .add_service_group(&[0, 1, 2, 3])
            .map_err(CircuitTestError::AddScabbardServiceError)?
            .unveil()
            .build()
            .map_err(CircuitTestError::CircuitBuildError)?;

        assert_eq!(result.roster.len(), 4);

        Ok(())
    }
}
