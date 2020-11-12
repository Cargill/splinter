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

#[cfg(all(feature = "benchmark", test))]
mod benchmarks;

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock};

use super::error::{
    AddCircuitError, AddCircuitsError, AddNodeError, AddNodesError, AddServiceError,
    RemoveCircuitError, RemoveNodeError, RemoveServiceError, RoutingTableReaderError,
};
use super::{
    Circuit, CircuitIter, CircuitNode, CircuitNodeIter, RoutingTableReader, RoutingTableWriter,
    Service, ServiceId,
};

use crate::error::{InternalError, InvalidStateError};

const ADMIN_CIRCUIT_ID: &str = "admin";

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
    /// * `service_id` - The unique ID for the service to be fetched
    ///
    /// Returns an error if the lock is poisoned.
    fn get_service(
        &self,
        service_id: &ServiceId,
    ) -> Result<Option<Service>, RoutingTableReaderError> {
        Ok(self
            .state
            .read()
            .map_err(|_| {
                RoutingTableReaderError::InternalError(InternalError::with_message(String::from(
                    "RoutingTable lock poisoned",
                )))
            })?
            .service_directory
            .get(service_id)
            .map(Service::clone))
    }

    /// Returns all the services for the provided circuit
    ///
    /// # Arguments
    ///
    /// * `circuit_id` - The unique ID the circuit whose services should be returned
    ///
    /// Returns an error if the lock is poisoned or if the circuit does not exist
    fn list_services(&self, circuit_id: &str) -> Result<Vec<Service>, RoutingTableReaderError> {
        if let Some(circuit) = self
            .state
            .read()
            .map_err(|_| {
                RoutingTableReaderError::InternalError(InternalError::with_message(String::from(
                    "RoutingTable lock poisoned",
                )))
            })?
            .circuits
            .get(circuit_id)
        {
            Ok(circuit.roster.clone())
        } else {
            Err(RoutingTableReaderError::InvalidStateError(
                InvalidStateError::with_message(format!("Circuit {} was not found", circuit_id)),
            ))
        }
    }

    // ---------- methods to access circuit directory ----------

    /// Returns the nodes in the routing table
    ///
    /// Returns an error if the lock is poisoned
    fn list_nodes(&self) -> Result<CircuitNodeIter, RoutingTableReaderError> {
        Ok(Box::new(
            self.state
                .read()
                .map_err(|_| {
                    RoutingTableReaderError::InternalError(InternalError::with_message(
                        String::from("RoutingTable lock poisoned"),
                    ))
                })?
                .nodes
                .clone()
                .into_iter(),
        ))
    }

    /// Returns the node with the provided ID
    ///
    /// # Arguments
    ///
    /// * `node_id` - The unique ID for the node to be fetched
    ///
    /// Returns an error if the lock was poisoned
    fn get_node(&self, node_id: &str) -> Result<Option<CircuitNode>, RoutingTableReaderError> {
        Ok(self
            .state
            .read()
            .map_err(|_| {
                RoutingTableReaderError::InternalError(InternalError::with_message(String::from(
                    "RoutingTable lock poisoned",
                )))
            })?
            .nodes
            .get(node_id)
            .cloned())
    }

    /// Returns the circuits in the routing table
    ///
    /// Returns an error if the lock is poisoned
    fn list_circuits(&self) -> Result<CircuitIter, RoutingTableReaderError> {
        Ok(Box::new(
            self.state
                .read()
                .map_err(|_| {
                    RoutingTableReaderError::InternalError(InternalError::with_message(
                        String::from("RoutingTable lock poisoned"),
                    ))
                })?
                .circuits
                .clone()
                .into_iter(),
        ))
    }

    /// Returns the circuit with the provided ID
    ///
    /// # Arguments
    ///
    /// * `circuit_id` - The unique ID for the circuit to be fetched
    ///
    /// Returns an error if the lock is poisoned
    fn get_circuit(&self, circuit_id: &str) -> Result<Option<Circuit>, RoutingTableReaderError> {
        if circuit_id == ADMIN_CIRCUIT_ID {
            Ok(Some(Circuit::new(
                ADMIN_CIRCUIT_ID.to_string(),
                vec![],
                vec![],
            )))
        } else {
            Ok(self
                .state
                .read()
                .map_err(|_| {
                    RoutingTableReaderError::InternalError(InternalError::with_message(
                        String::from("RoutingTable lock poisoned"),
                    ))
                })?
                .circuits
                .get(circuit_id)
                .cloned())
        }
    }

    fn clone_boxed(&self) -> Box<dyn RoutingTableReader> {
        Box::new(self.clone())
    }
}

impl RoutingTableWriter for RoutingTable {
    /// Adds a new service to the routing table
    ///
    /// # Arguments
    ///
    /// * `service_id` - The unique ServiceId for the service
    /// * `service` - The service to be added to the routing table
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
    /// * `service_id` - The unique ServiceId for the service
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
    /// * `circuit_id` - The unique ID for the circuit
    /// * `circuit` - The circuit to be added to the routing table
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
    /// * `circuit_id` - The unique ID for the circuit
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
    /// * `node_id` - The unique ID for the node
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

    fn clone_boxed(&self) -> Box<dyn RoutingTableWriter> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    // Test the routing table read and write operations for circuits
    //
    // 1. Create circuits with corresponding nodes and services and write one circuit to the
    //    routing table
    // 2. Check that the circuit was written to the routing table
    // 3. Check that the expected circuit is returned when fetched
    // 4. Remove the circuit from the routing table
    // 5. Check that the 'circuits' field of the routing table is empty
    // 6. Write both circuits to the routing table
    // 7. Check that both circuits were written to the routing table
    // 8. list circuits, validate both circuits are returned
    #[test]
    fn test_circuit() {
        let routing_table = RoutingTable::default();
        let mut writer: Box<dyn RoutingTableWriter> = Box::new(routing_table.clone());
        let reader: Box<dyn RoutingTableReader> = Box::new(routing_table.clone());

        // create four nodes and four services
        let mut roster = vec![];
        let mut nodes = vec![];
        let mut members = vec![];
        for x in 0..4 {
            let node = CircuitNode {
                node_id: format!("node-{}", x),
                endpoints: vec![format!("endpoint_{}", x)],
            };
            let service = Service {
                service_id: format!("service-{}", x),
                service_type: "test".to_string(),
                node_id: format!("endpoint_{}", x),
                arguments: vec![("peer_services".to_string(), "node-000".to_string())],
                peer_id: None,
            };
            roster.push(service.clone());
            nodes.push(node.clone());
            members.push(node.node_id.clone());
        }
        let circuit_roster0 = vec![roster[0].clone(), roster[1].clone()];
        let circuit_roster1 = vec![roster[2].clone(), roster[3].clone()];
        let circuit_members0 = vec![members[0].clone(), members[1].clone()];
        let circuit_members1 = vec![members[2].clone(), members[3].clone()];
        let circuit_nodes0 = vec![nodes[0].clone(), nodes[1].clone()];

        // create circuits with the previously created nodes and services
        let circuit0 = Circuit {
            circuit_id: "012-abc".to_string(),
            roster: circuit_roster0.clone(),
            members: circuit_members0.clone(),
        };
        let circuit1 = Circuit {
            circuit_id: "345-def".to_string(),
            roster: circuit_roster1.clone(),
            members: circuit_members1.clone(),
        };

        let mut expected_nodes = BTreeMap::new();
        let mut expected_circuits = BTreeMap::new();
        let mut expected_service_directory = HashMap::new();

        expected_nodes.insert(nodes[0].node_id.clone().to_string(), nodes[0].clone());
        expected_nodes.insert(nodes[1].node_id.clone().to_string(), nodes[1].clone());

        expected_circuits.insert(circuit0.circuit_id.clone().to_string(), circuit0.clone());

        expected_service_directory.insert(
            ServiceId::new(
                "012-abc".to_string(),
                circuit_roster0[0].service_id.clone().to_string(),
            ),
            circuit_roster0[0].clone(),
        );
        expected_service_directory.insert(
            ServiceId::new(
                "012-abc".to_string(),
                circuit_roster0[1].service_id.clone().to_string(),
            ),
            circuit_roster0[1].clone(),
        );

        // add a circuit to the routing table
        writer
            .add_circuit(
                "012-abc".to_string(),
                circuit0.clone(),
                circuit_nodes0.clone(),
            )
            .expect("Unable to add circuit");

        assert_eq!(routing_table.state.read().unwrap().nodes, expected_nodes);
        assert_eq!(
            routing_table.state.read().unwrap().circuits,
            expected_circuits
        );
        assert_eq!(
            routing_table.state.read().unwrap().service_directory,
            expected_service_directory
        );

        // remove circuit from the routing table
        writer
            .remove_circuit("012-abc")
            .expect("Unable to remove circuit");

        assert!(!routing_table.state.read().unwrap().nodes.is_empty());
        assert!(routing_table.state.read().unwrap().circuits.is_empty());
        assert!(routing_table
            .state
            .read()
            .unwrap()
            .service_directory
            .is_empty());

        expected_circuits.insert(circuit1.circuit_id.clone().to_string(), circuit1.clone());

        expected_service_directory.insert(
            ServiceId::new(
                "345-def".to_string(),
                circuit_roster1[0].service_id.clone().to_string(),
            ),
            circuit_roster1[0].clone(),
        );
        expected_service_directory.insert(
            ServiceId::new(
                "345-def".to_string(),
                circuit_roster1[1].service_id.clone().to_string(),
            ),
            circuit_roster1[1].clone(),
        );

        // add multiple circuits to the routing table
        writer
            .add_circuits(vec![circuit0.clone(), circuit1.clone()])
            .expect("Unable to add circuits");

        assert_eq!(routing_table.state.read().unwrap().nodes, expected_nodes);
        assert_eq!(
            routing_table.state.read().unwrap().circuits,
            expected_circuits
        );
        assert_eq!(
            routing_table.state.read().unwrap().service_directory,
            expected_service_directory
        );

        // get one of the circuits in the routing table
        let fetched_circuit = reader
            .get_circuit(&circuit0.circuit_id)
            .expect("Unable to get circuit");

        assert_eq!(fetched_circuit, Some(circuit0));

        // list all circuits in the routing table
        let fetched_circuit_list = reader.list_circuits().expect("Unable to list circuits");

        assert_eq!(
            fetched_circuit_list.collect::<BTreeMap<String, Circuit>>(),
            expected_circuits
        );
    }

    // Test the routing table read and write operations for services
    //
    // 1. Create a circuit with corresponding nodes and services
    // 2. Write a service to the routing table
    // 3. Check the service was written to the routing table
    // 4. Remove service from the routing table
    // 5. Add circuit with two services to the routing table, validate ok
    // 6. Check the expected service is returned when fetched
    // 7. List services, validate both services are returned
    #[test]
    fn test_service() {
        let routing_table = RoutingTable::default();
        let mut writer: Box<dyn RoutingTableWriter> = Box::new(routing_table.clone());
        let reader: Box<dyn RoutingTableReader> = Box::new(routing_table.clone());

        // create two nodes, two services, and one circuit
        let node0 = CircuitNode {
            node_id: "node-0".to_string(),
            endpoints: vec!["endpoint_0".to_string()],
        };
        let service0 = Service {
            service_id: "service-0".to_string(),
            service_type: "test".to_string(),
            node_id: "endpoint_0".to_string(),
            arguments: vec![("peer_services".to_string(), "node-000".to_string())],
            peer_id: None,
        };
        let node1 = CircuitNode {
            node_id: "node-1".to_string(),
            endpoints: vec!["endpoint_1".to_string()],
        };
        let service1 = Service {
            service_id: "service-1".to_string(),
            service_type: "test".to_string(),
            node_id: "endpoint_1".to_string(),
            arguments: vec![("peer_services".to_string(), "node-000".to_string())],
            peer_id: None,
        };
        let circuit = Circuit {
            circuit_id: "012-abc".to_string(),
            roster: vec![service0.clone(), service1.clone()],
            members: vec![node0.node_id.clone(), node1.node_id.clone()],
        };
        let service_id0 = ServiceId::new(
            "012-abc".to_string(),
            service0.service_id.clone().to_string(),
        );

        let mut expected_service_directory = HashMap::new();

        let expected_service_id = ServiceId::new("012-abc".to_string(), "service-0".to_string());
        expected_service_directory.insert(expected_service_id, service0.clone());

        // add service to the routing table
        writer
            .add_service(service_id0.clone(), service0.clone())
            .expect("Unable to add service");

        assert_eq!(
            routing_table.state.read().unwrap().service_directory,
            expected_service_directory
        );

        // remove service from the routing table
        writer
            .remove_service(&service_id0.clone())
            .expect("Unable to remove service");

        assert!(routing_table
            .state
            .read()
            .unwrap()
            .service_directory
            .is_empty());

        // add circuit with two services to the routing table
        writer
            .add_circuit(
                circuit.circuit_id.clone(),
                circuit.clone(),
                vec![node0.clone(), node1.clone()],
            )
            .expect("Unable to add circuit");

        assert!(routing_table
            .state
            .read()
            .unwrap()
            .circuits
            .contains_key("012-abc"));

        // get one of the services in the routing table
        let fetched_service = reader
            .get_service(&service_id0.clone())
            .expect("Unable to get service");

        assert_eq!(fetched_service, Some(service0.clone()));

        // list all services in the routing table
        let fetched_service_list = reader
            .list_services(&circuit.circuit_id)
            .expect("Unable to list services");

        assert_eq!(fetched_service_list, vec![service0, service1]);
    }

    // Test the routing table read and write operations for nodes
    //
    // 1. Create two nodes, write one node to the routing table
    // 2. Check node was written to the routing table
    // 3. Remove node from the routing table
    // 4. Check node was removed from the routing table
    // 5. Add two nodes to the routing table
    // 6. Check both nodes written to the routing table
    // 7. Check the expected node is returned when fetched
    // 8. List nodes, validate both nodes are returned
    #[test]
    fn test_node() {
        let routing_table = RoutingTable::default();
        let mut writer: Box<dyn RoutingTableWriter> = Box::new(routing_table.clone());
        let reader: Box<dyn RoutingTableReader> = Box::new(routing_table.clone());

        let node0 = CircuitNode {
            node_id: "node-0".to_string(),
            endpoints: vec!["endpoint_0".to_string()],
        };
        let node1 = CircuitNode {
            node_id: "node-1".to_string(),
            endpoints: vec!["endpoint_1".to_string()],
        };

        let mut expected_nodes = BTreeMap::new();
        expected_nodes.insert(node0.node_id.clone(), node0.clone());

        // add node to the routing table
        writer
            .add_node(node0.node_id.clone(), node0.clone())
            .expect("Unable to add node");

        assert_eq!(routing_table.state.read().unwrap().nodes, expected_nodes);

        // remove node from the routing table
        writer
            .remove_node(&node0.node_id)
            .expect("Unable to remove node");

        assert!(routing_table.state.read().unwrap().nodes.is_empty());

        // add multiple nodes to the routing table
        writer
            .add_nodes(vec![node0.clone(), node1.clone()])
            .expect("Unable to add nodes");

        expected_nodes.insert(node1.node_id.clone(), node1.clone());

        assert_eq!(routing_table.state.read().unwrap().nodes, expected_nodes);

        // get a node from the routing table
        let fetched_node = reader
            .get_node(&node0.node_id.clone())
            .expect("Unable to get node");

        assert_eq!(fetched_node, Some(node0.clone()));

        // list all nodes in the routing table
        let fetched_node_list = reader.list_nodes().expect("Unable to list nodes");

        assert_eq!(
            fetched_node_list.collect::<BTreeMap<String, CircuitNode>>(),
            expected_nodes
        );
    }
}
