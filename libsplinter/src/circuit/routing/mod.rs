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

//! A routing table for directing messages to nodes and services based on defined circuits.
//!
//! The routing table stores information required for routing messages to nodes and services that
//! are a part of a circuit. The routing table is split into two traits, a reader and a writer.
//! A writer is used to update the routing table with circuit, node, and service routing
//! information. For example, the `AdminService` uses a writer when a new circuit has been added to
//! Splinter state. Components that require routing information must use a reader. For example,
//! the dispatch handlers use the reader to route messages to services or other nodes on a circuit.
//!
//! The public interface includes the traits [`RoutingTableReader`] and [`RoutingTableWriter`] and
//! the structs [`Service`], [`ServiceId`], [`Circuit`], and [`CircuitNode`]. It also includes
//! a RwLock implmentation of the traits [`RoutingTable`].
//!
//! [`Circuit`]: struct.Circuit.html
//! [`CircuitNode`]: struct.CircuitNode.html
//! [`RoutingTable`]: memory/struct.RoutingTable.html
//! [`RoutingTableReader`]: trait.RoutingTableReader.html
//! [`RoutingTableWriter`]: trait.RoutingTableWriter.html
//! [`Service`]: struct.Service.html
//! [`ServiceId`]: struct.ServiceId.html

mod error;
pub mod memory;

use std::cmp::Ordering;
use std::fmt;

use self::error::RoutingTableReaderError;

use crate::error::InternalError;

/// Interface for updating the routing table
pub trait RoutingTableWriter: Send {
    /// Adds a new service to the routing table
    ///
    /// # Arguments
    ///
    /// * `service_id` - The unique ServiceId for the service
    /// * `service` - The service to be added to the routing table
    fn add_service(&mut self, service_id: ServiceId, service: Service)
        -> Result<(), InternalError>;

    /// Removes a service from the routing table if it exists
    ///
    /// # Arguments
    ///
    /// * `service_id` - The unique ServiceId for the service
    fn remove_service(&mut self, service_id: &ServiceId) -> Result<(), InternalError>;

    /// Adds a new circuit to the routing table. Also adds the associated services and nodes.
    ///
    /// # Arguments
    ///
    /// * `circuit_id` - The unique ID for the circuit
    /// * `circuit` - The circuit to be added to the routing table
    /// * `nodes` - The list of circuit nodes that should be added along with the circuit
    fn add_circuit(
        &mut self,
        circuit_id: String,
        circuit: Circuit,
        nodes: Vec<CircuitNode>,
    ) -> Result<(), InternalError>;

    /// Adds a list of circuits to the routing table. Also adds the associated services.
    ///
    /// # Arguments
    ///
    /// * `circuits` - The list of circuits to be added to the routing table
    fn add_circuits(&mut self, circuits: Vec<Circuit>) -> Result<(), InternalError>;

    /// Removes a circuit from the routing table if it exists. Also removes the associated
    /// services.
    ///
    /// # Arguments
    ///
    /// * `circuit_id` - The unique ID for the circuit
    fn remove_circuit(&mut self, circuit_id: &str) -> Result<(), InternalError>;

    /// Adds a new node to the routing table
    ///
    /// # Arguments
    ///
    /// * `node_id` - The unique ID for the node
    /// * `node`- The node to add to the routing table
    fn add_node(&mut self, node_id: String, node: CircuitNode) -> Result<(), InternalError>;

    /// Adds a list of node to the routing table
    ///
    /// # Arguments
    ///
    /// * `nodes`- The list of nodes to add to the routing table
    fn add_nodes(&mut self, nodes: Vec<CircuitNode>) -> Result<(), InternalError>;

    /// Removes a node from the routing table if it exists
    ///
    /// # Arguments
    ///
    /// * `node_id` - The unique ID for the node that should be removed
    fn remove_node(&mut self, node_id: &str) -> Result<(), InternalError>;

    fn clone_boxed(&self) -> Box<dyn RoutingTableWriter>;
}

impl Clone for Box<dyn RoutingTableWriter> {
    fn clone(&self) -> Self {
        self.clone_boxed()
    }
}

/// Type returned by the `RoutingTableReader::list_nodes` method
pub type CircuitNodeIter = Box<dyn ExactSizeIterator<Item = (String, CircuitNode)> + Send>;

/// Type returned by the `RoutingTableReader::list_circuits` method
pub type CircuitIter = Box<dyn ExactSizeIterator<Item = (String, Circuit)> + Send>;

/// Interface for reading the routing table
pub trait RoutingTableReader: Send {
    // ---------- methods to access service directory ----------

    /// Returns the service with the provided ID
    ///
    /// # Arguments
    ///
    /// * `service_id` - The unique ID for the service to be fetched
    fn get_service(
        &self,
        service_id: &ServiceId,
    ) -> Result<Option<Service>, RoutingTableReaderError>;

    /// Returns all the services for the provided circuit
    ///
    /// # Arguments
    ///
    /// * `circuit_id` - The unique ID the circuit whose services should be returned
    fn list_services(&self, circuit_id: &str) -> Result<Vec<Service>, RoutingTableReaderError>;

    // ---------- methods to access circuit directory ----------

    /// Returns the nodes in the routing table
    fn list_nodes(&self) -> Result<CircuitNodeIter, RoutingTableReaderError>;

    /// Returns the node with the provided ID
    ///
    /// # Arguments
    ///
    /// * `node_id` - The unique ID for the node to be fetched
    fn get_node(&self, node_id: &str) -> Result<Option<CircuitNode>, RoutingTableReaderError>;

    /// Returns the circuits in the routing table
    fn list_circuits(&self) -> Result<CircuitIter, RoutingTableReaderError>;

    /// Returns the circuit with the provided ID
    ///
    /// # Arguments
    ///
    /// * `circuit_id` - The unique ID for the circuit to be fetched
    fn get_circuit(&self, circuit_id: &str) -> Result<Option<Circuit>, RoutingTableReaderError>;

    fn clone_boxed(&self) -> Box<dyn RoutingTableReader>;
}

impl Clone for Box<dyn RoutingTableReader> {
    fn clone(&self) -> Self {
        self.clone_boxed()
    }
}

/// The routing table representation of a circuit. It is simplified to only contain the required
/// values for routing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Circuit {
    circuit_id: String,
    roster: Vec<Service>,
    members: Vec<String>,
}

impl Circuit {
    /// Creates a new `Circuit`
    ///
    /// # Arguments
    ///
    /// * `circuit_id` -  The unique ID for the circuit
    /// * `roster` - The list of services in the circuit
    /// * `members` - The list of node IDs for the members of a circuit
    pub fn new(circuit_id: String, roster: Vec<Service>, members: Vec<String>) -> Self {
        Circuit {
            circuit_id,
            roster,
            members,
        }
    }

    /// Returns the ID of the circuit
    pub fn circuit_id(&self) -> &str {
        &self.circuit_id
    }

    /// Returns the list of service that are in the circuit
    pub fn roster(&self) -> &[Service] {
        &self.roster
    }

    /// Returns the list of node IDs that are in the circuit
    pub fn members(&self) -> &[String] {
        &self.members
    }
}

/// The routing table representation of a node
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CircuitNode {
    node_id: String,
    endpoints: Vec<String>,
}

impl CircuitNode {
    /// Creates a new `CircuitNode`
    ///
    /// # Arguments
    ///
    /// * `node_id` -  The unique ID for the circuit
    /// * `endpoints` -  A list of endpoints the node can be reached at
    pub fn new(node_id: String, endpoints: Vec<String>) -> Self {
        CircuitNode { node_id, endpoints }
    }
}

impl Ord for CircuitNode {
    fn cmp(&self, other: &Self) -> Ordering {
        self.node_id.cmp(&other.node_id)
    }
}

impl PartialOrd for CircuitNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// The routing table representation of a service
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Service {
    service_id: String,
    service_type: String,
    node_id: String,
    arguments: Vec<(String, String)>,
    peer_id: Option<String>,
}

impl Service {
    /// Creates a new `Service`
    ///
    /// # Arguments
    ///
    /// * `service_id` -  The unique ID for the service
    /// * `service_type` - The type of service this is
    /// * `node_id` - The node ID that this service can connect to
    /// * `arguments` - The key-value pairs of arguments that will be passed to the service
    pub fn new(
        service_id: String,
        service_type: String,
        node_id: String,
        arguments: Vec<(String, String)>,
    ) -> Self {
        Service {
            service_id,
            service_type,
            node_id,
            arguments,
            peer_id: None,
        }
    }

    /// Returns the ID of the service
    pub fn service_id(&self) -> &str {
        &self.service_id
    }

    /// Returns the service type of the service
    pub fn service_type(&self) -> &str {
        &self.service_type
    }

    /// Returns the node ID of the node the service can connect to
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Returns the list of key/value arugments for the service
    pub fn arguments(&self) -> &[(String, String)] {
        &self.arguments
    }

    /// Returns the local peer ID for the service
    pub fn peer_id(&self) -> &Option<String> {
        &self.peer_id
    }

    pub fn set_peer_id(&mut self, peer_id: String) {
        self.peer_id = Some(peer_id)
    }

    pub fn remove_peer_id(&mut self) {
        self.peer_id = None
    }
}

/// The unique ID of a service made up of a circuit ID and service ID
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ServiceId {
    circuit_id: String,
    service_id: String,
}

impl ServiceId {
    /// Creates a new `ServiceId`
    ///
    /// # Arguments
    ///
    /// * `circuit_id` -  The unique ID for the circuit this service belongs to
    /// * `service_id` -  The unique ID for the service
    pub fn new(circuit_id: String, service_id: String) -> Self {
        ServiceId {
            circuit_id,
            service_id,
        }
    }

    /// Returns the circuit ID
    pub fn circuit(&self) -> &str {
        &self.circuit_id
    }

    /// Returns the service ID
    pub fn service_id(&self) -> &str {
        &self.service_id
    }

    /// Decompose the service ID into a tuple of (<circuit ID>, <service ID>).
    pub fn into_parts(self) -> (String, String) {
        (self.circuit_id, self.service_id)
    }
}

impl fmt::Display for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}::{}", self.circuit_id, self.service_id)
    }
}

impl Eq for ServiceId {}
