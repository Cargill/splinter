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

//! Implementation of the circuit routing table reader and writer traits that uses a read/write
//! lock
//!
//! The public interface includes the structs [`RoutingTable`].
//!
//! [`RoutingTable`]: struct.RoutingTable.html

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock};

use super::error::{
    AddCircuitError, AddCircuitsError, AddNodeError, AddNodesError, AddServiceError,
    FetchCircuitError, FetchNodeError, FetchServiceError, ListCircuitsError, ListNodesError,
    ListServiceError, RemoveCircuitError, RemoveNodeError, RemoveServiceError,
};
use super::{
    Circuit, CircuitIter, CircuitNode, CircuitNodeIter, RoutingTableReader, RoutingTableWriter,
    Service, ServiceId,
};

/// The internal state of the routing table that will be wrapped in a read-write lock
#[derive(Clone, Default)]
struct RoutingTableState {
    /// Nodes that are listed in a set of circuits
    nodes: BTreeMap<String, CircuitNode>,
    /// The collection of circuits to be used for routing
    circuits: BTreeMap<String, Circuit>,
    /// Service ID to Service that contains the node the service is connected to. Not persisted.
    service_directory: HashMap<ServiceId, Service>,
}

// An implementation of a routing table that uses a read-write lock to wrap the state.
#[derive(Clone, Default)]
pub struct RoutingTable {
    state: Arc<RwLock<RoutingTableState>>,
}

impl RoutingTableReader for RoutingTable {
    // ---------- methods to access service directory ----------
    /// Returns the service with the provided ID
    ///
    /// # Arguments
    ///
    /// * `service_id` -  The unique ID for the service to be fetched
    ///
    /// Returns an error if the lock is poisoned.
    fn fetch_service(&self, service_id: &ServiceId) -> Result<Option<Service>, FetchServiceError> {
        Ok(self
            .state
            .read()
            .map_err(|_| FetchServiceError(String::from("RoutingTable lock poisoned")))?
            .service_directory
            .get(service_id)
            .map(Service::clone))
    }

    /// Returns all the services for the provided circuit
    ///
    /// # Arguments
    ///
    /// * `circuit_id` -  The unique ID the circuit whose services should be returned
    ///
    /// Returns an error if the lock is poisoned or if the circuit does not exist
    fn list_service(&self, circuit_id: &str) -> Result<Vec<Service>, ListServiceError> {
        if let Some(circuit) = self
            .state
            .read()
            .map_err(|_| {
                ListServiceError::InternalError(String::from("RoutingTable lock poisoned"))
            })?
            .circuits
            .get(circuit_id)
        {
            Ok(circuit.roster.clone())
        } else {
            Err(ListServiceError::CircuitNotFound(circuit_id.to_string()))
        }
    }

    // ---------- methods to access circuit directory ----------

    /// Returns the nodes in the routing table
    ///
    /// Returns an error if the lock is poisoned
    fn list_nodes(&self) -> Result<CircuitNodeIter, ListNodesError> {
        Ok(Box::new(
            self.state
                .read()
                .map_err(|_| ListNodesError(String::from("RoutingTable lock poisoned")))?
                .nodes
                .clone()
                .into_iter(),
        ))
    }

    /// Returns the node with the provided ID
    ///
    /// # Arguments
    ///
    /// * `node_id` -  The unique ID for the node to be fetched
    ///
    /// Returns an error if the lock was poisoned
    fn fetch_node(&self, node_id: &str) -> Result<Option<CircuitNode>, FetchNodeError> {
        Ok(self
            .state
            .read()
            .map_err(|_| FetchNodeError(String::from("RoutingTable lock poisoned")))?
            .nodes
            .get(node_id)
            .cloned())
    }

    /// Returns the circuits in the routing table
    ///
    /// Returns an error if the lock is poisoned
    fn list_circuits(&self) -> Result<CircuitIter, ListCircuitsError> {
        Ok(Box::new(
            self.state
                .read()
                .map_err(|_| ListCircuitsError(String::from("RoutingTable lock poisoned")))?
                .circuits
                .clone()
                .into_iter(),
        ))
    }

    /// Returns the circuit with the provided ID
    ///
    /// # Arguments
    ///
    /// * `circuit_id` -  The unique ID for the circuit to be fetched
    ///
    /// Returns an error if the lock is poisoned
    fn fetch_circuit(&self, circuit_id: &str) -> Result<Option<Circuit>, FetchCircuitError> {
        Ok(self
            .state
            .read()
            .map_err(|_| FetchCircuitError(String::from("RoutingTable lock poisoned")))?
            .circuits
            .get(circuit_id)
            .cloned())
    }
}

impl RoutingTableWriter for RoutingTable {
    /// Adds a new service to the routing table
    ///
    /// # Arguments
    ///
    /// * `service_id` -  The unique ServiceId for the service
    /// * `service` -  The service to be added to the routing table
    ///
    /// Returns an error if the lock is poisoned
    fn add_service(
        &mut self,
        service_id: ServiceId,
        service: Service,
    ) -> Result<(), AddServiceError> {
        self.state
            .write()
            .map_err(|_| AddServiceError(String::from("RoutingTable lock poisoned")))?
            .service_directory
            .insert(service_id, service);
        Ok(())
    }

    /// Removes a service from the routing table if it exists
    ///
    /// # Arguments
    ///
    /// * `service_id` -  The unique ServiceId for the service
    ///
    /// Returns an error if the lock is poisoned
    fn remove_service(&mut self, service_id: &ServiceId) -> Result<(), RemoveServiceError> {
        self.state
            .write()
            .map_err(|_| RemoveServiceError(String::from("RoutingTable lock poisoned")))?
            .service_directory
            .remove(service_id);
        Ok(())
    }

    /// Adds a new circuit to the routing table. Also adds the associated services and nodes.
    ///
    /// # Arguments
    ///
    /// * `circuit_id` -  The unique ID for the circuit
    /// * `circuit` -  The circuit to be added to the routing table
    /// * `nodes` - The list of circuit nodes that should be added along with the circuit
    ///
    /// Returns an error if the lock is poisoned
    fn add_circuit(
        &mut self,
        circuit_id: String,
        circuit: Circuit,
        nodes: Vec<CircuitNode>,
    ) -> Result<(), AddCircuitError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| AddCircuitError(String::from("RoutingTable lock poisoned")))?;

        for service in circuit.roster.iter() {
            let service_id = ServiceId::new(
                circuit.circuit_id.to_string(),
                service.service_id.to_string(),
            );

            state.service_directory.insert(service_id, service.clone());
        }

        for node in nodes.into_iter() {
            if !state.nodes.contains_key(&node.node_id) {
                state.nodes.insert(node.node_id.to_string(), node);
            }
        }

        state.circuits.insert(circuit_id, circuit);
        Ok(())
    }

    /// Adds a list of circuits to the routing table. Also adds the associated services.
    ///
    /// # Arguments
    ///
    /// * `circuits` - The list of circuits to be added to the routing table
    ///
    /// Returns an error if the lock is poisoned
    fn add_circuits(&mut self, circuits: Vec<Circuit>) -> Result<(), AddCircuitsError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| AddCircuitsError(String::from("RoutingTable lock poisoned")))?;
        for circuit in circuits.into_iter() {
            for service in circuit.roster.iter() {
                let service_id = ServiceId::new(
                    circuit.circuit_id.to_string(),
                    service.service_id.to_string(),
                );

                state.service_directory.insert(service_id, service.clone());
            }
            state
                .circuits
                .insert(circuit.circuit_id.to_string(), circuit);
        }
        Ok(())
    }

    /// Removes a circuit from the routing table if it exists. Also removes the associated
    /// services.
    ///
    /// # Arguments
    ///
    /// * `circuit_id` -  The unique ID for the circuit
    ///
    /// Returns an error if the lock is poisoned
    fn remove_circuit(&mut self, circuit_id: &str) -> Result<(), RemoveCircuitError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| RemoveCircuitError(String::from("RoutingTable lock poisoned")))?;

        let circuit = state.circuits.remove(circuit_id);

        if let Some(circuit) = circuit {
            for service in circuit.roster.iter() {
                let service_id = ServiceId::new(
                    circuit.circuit_id.to_string(),
                    service.service_id.to_string(),
                );

                state.service_directory.remove(&service_id);
            }
        }
        Ok(())
    }

    /// Adds a new node to the routing table
    ///
    /// # Arguments
    ///
    /// * `node_id` -  The unique ID for the node
    /// * `node`- The node to add to the routing table
    ///
    /// Returns an error if the lock is poisoned
    fn add_node(&mut self, id: String, node: CircuitNode) -> Result<(), AddNodeError> {
        self.state
            .write()
            .map_err(|_| AddNodeError(String::from("RoutingTable lock poisoned")))?
            .nodes
            .insert(id, node);
        Ok(())
    }

    /// Adds a list of node to the routing table
    ///
    /// # Arguments
    ///
    /// * `nodes`- The list of nodes to add to the routing table
    ///
    /// Returns an error if the lock is poisoned
    fn add_nodes(&mut self, nodes: Vec<CircuitNode>) -> Result<(), AddNodesError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| AddNodesError(String::from("RoutingTable lock poisoned")))?;
        for node in nodes {
            if !state.nodes.contains_key(&node.node_id) {
                state.nodes.insert(node.node_id.to_string(), node);
            }
        }
        Ok(())
    }

    /// Removes a node from the routing table if it exists
    ///
    /// # Arguments
    ///
    /// * `node_id` -  The unique ID for the node that should be removed
    ///
    /// Returns an error if the lock is poisoned
    fn remove_node(&mut self, id: &str) -> Result<(), RemoveNodeError> {
        self.state
            .write()
            .map_err(|_| RemoveNodeError(String::from("RoutingTable lock poisoned")))?
            .nodes
            .remove(id);
        Ok(())
    }
}
