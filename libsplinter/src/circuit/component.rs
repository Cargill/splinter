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

//! trait implementations to support service components.

use crate::circuit::routing::{RoutingTableReader, RoutingTableWriter, Service, ServiceId};
use crate::service::network::handlers::{
    ServiceAddInstanceError, ServiceInstances, ServiceRemoveInstanceError,
};

const ADMIN_SERVICE_ID_PREFIX: &str = "admin::";
const ADMIN_CIRCUIT_ID: &str = "admin";

/// A collection of service instances, backed by `RoutingTable`.
pub struct RoutingTableServiceInstances {
    node_id: String,
    routing_table_reader: Box<dyn RoutingTableReader>,
    routing_table_writer: Box<dyn RoutingTableWriter>,
}

impl RoutingTableServiceInstances {
    /// Construct a new instance with the given local node information and an instance of splinter
    /// state.
    ///
    /// # Params
    ///
    /// - `node_id`: the local node information
    /// - `routing_table_reader`: reader to check existing service state
    /// - `routing_table_writer`: writer to update service state
    pub fn new(
        node_id: String,
        routing_table_reader: Box<dyn RoutingTableReader>,
        routing_table_writer: Box<dyn RoutingTableWriter>,
    ) -> Self {
        Self {
            node_id,
            routing_table_reader,
            routing_table_writer,
        }
    }
}

impl ServiceInstances for RoutingTableServiceInstances {
    fn add_service_instance(
        &self,
        service_id: ServiceId,
        component_id: String,
    ) -> Result<(), ServiceAddInstanceError> {
        let unique_id = service_id.clone();
        let (circuit_name, service_id) = service_id.into_parts();

        let circuit = self
            .routing_table_reader
            .get_circuit(&circuit_name)
            .map_err(|err| ServiceAddInstanceError::InternalError {
                context: err.reduce_to_string(),
                source: None,
            })?
            .ok_or(ServiceAddInstanceError::CircuitDoesNotExist)?;

        if !circuit
            .roster()
            .iter()
            .any(|service| service.service_id() == service_id)
        {
            return Err(ServiceAddInstanceError::NotInCircuit);
        }

        let mut service = if !service_id.starts_with(ADMIN_SERVICE_ID_PREFIX) {
            self.routing_table_reader
                .get_service(&unique_id)
                .map_err(|err| ServiceAddInstanceError::InternalError {
                    context: err.reduce_to_string(),
                    source: None,
                })?
                .ok_or(ServiceAddInstanceError::NotInCircuit)?
        } else {
            Service::new(
                service_id,
                ADMIN_CIRCUIT_ID.to_string(),
                self.node_id.to_string(),
                vec![],
            )
        };

        if service.node_id() != self.node_id {
            return Err(ServiceAddInstanceError::NotAllowed);
        }

        if service.peer_id().is_some() {
            return Err(ServiceAddInstanceError::AlreadyRegistered);
        }

        service.set_peer_id(component_id);

        let mut writer = self.routing_table_writer.clone();
        writer.add_service(unique_id, service).map_err(|err| {
            ServiceAddInstanceError::InternalError {
                context: err.reduce_to_string(),
                source: None,
            }
        })?;
        Ok(())
    }

    fn remove_service_instance(
        &self,
        service_id: ServiceId,
        _component_id: String,
    ) -> Result<(), ServiceRemoveInstanceError> {
        let mut service = self
            .routing_table_reader
            .get_service(&service_id)
            .map_err(|err| ServiceRemoveInstanceError::InternalError {
                context: err.reduce_to_string(),
                source: None,
            })?
            .ok_or(ServiceRemoveInstanceError::NotRegistered)?;

        if service.peer_id().is_none() {
            return Err(ServiceRemoveInstanceError::NotRegistered);
        }

        service.remove_peer_id();

        let mut writer = self.routing_table_writer.clone();
        writer.add_service(service_id, service).map_err(|err| {
            ServiceRemoveInstanceError::InternalError {
                context: err.reduce_to_string(),
                source: None,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::circuit::routing::{
        memory::RoutingTable, Circuit, CircuitNode, RoutingTableWriter, Service,
    };

    #[test]
    // Test that if the circuit does not exist, a ServiceAddInstanceError::CircuitDoesNotExist
    // error is returned.
    fn test_add_service_instance_circuit_does_not_exist() {
        let table = RoutingTable::default();
        let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());
        let service_instances =
            RoutingTableServiceInstances::new("123".to_string(), reader, writer);

        let res = service_instances.add_service_instance(
            ServiceId::new("alpha".into(), "abc".into()),
            "my_component".into(),
        );

        assert!(matches!(
            res,
            Err(ServiceAddInstanceError::CircuitDoesNotExist)
        ));
    }

    #[test]
    // Test that if the service is not in circuit, a ServiceAddInstanceError::NotInCircuit error is
    // returned.
    fn test_add_service_instance_not_in_circuit() {
        let (circuit, nodes) = build_circuit();

        let table = RoutingTable::default();
        let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
        let mut writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        writer
            .add_circuit(circuit.circuit_id().into(), circuit, nodes)
            .expect("Unable to add circuit");

        let service_instances =
            RoutingTableServiceInstances::new("123".to_string(), reader, writer);

        let res = service_instances.add_service_instance(
            ServiceId::new("alpha".into(), "BAD".into()),
            "my_component".into(),
        );

        assert!(matches!(res, Err(ServiceAddInstanceError::NotInCircuit)));
    }

    #[test]
    // Test that if the service is already registered, a ServiceAddInstanceError::AlreadyRegistered
    // error is returned.
    fn test_add_service_instance_already_registered() {
        let (circuit, nodes) = build_circuit();

        let table = RoutingTable::default();
        let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
        let mut writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        writer
            .add_circuit(circuit.circuit_id().into(), circuit, nodes)
            .expect("Unable to add circuit");

        let id = ServiceId::new("alpha".into(), "abc".into());
        let mut service = reader
            .get_service(&id)
            .expect("Unable to get service")
            .expect("Missing service");
        service.set_peer_id("abc_network".into());
        writer
            .add_service(id, service)
            .expect("Unable to add service");

        let service_instances =
            RoutingTableServiceInstances::new("123".to_string(), reader, writer);

        let res = service_instances.add_service_instance(
            ServiceId::new("alpha".into(), "abc".into()),
            "my_component".into(),
        );

        assert!(matches!(
            res,
            Err(ServiceAddInstanceError::AlreadyRegistered)
        ));
    }

    #[test]
    // Test that if the service is in a circuit and not connected, the service is accepted.
    // This is the happy-path test
    fn test_add_service_instance_accepted() {
        let (circuit, nodes) = build_circuit();

        let table = RoutingTable::default();
        let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
        let mut writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        writer
            .add_circuit(circuit.circuit_id().into(), circuit, nodes)
            .expect("Unable to add circuit");

        let service_instances =
            RoutingTableServiceInstances::new("123".to_string(), reader.clone(), writer);

        let id = ServiceId::new("alpha".into(), "abc".into());
        let res = service_instances.add_service_instance(id.clone(), "my_component".into());

        assert!(matches!(res, Ok(())));
        assert_eq!(
            reader
                .get_service(&id)
                .expect("cannot check if it has the service")
                .expect("no service returned")
                .peer_id(),
            &Some("my_component".to_string())
        );
    }

    #[test]
    // Test that if the circuit is not registered, a ServiceRemoveInstanceError::NotRegistered
    // should be returned.
    fn test_remove_service_instance_not_registred() {
        let (circuit, nodes) = build_circuit();

        let table = RoutingTable::default();
        let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
        let mut writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        writer
            .add_circuit(circuit.circuit_id().into(), circuit, nodes)
            .expect("Unable to add circuit");

        let service_instances =
            RoutingTableServiceInstances::new("123".to_string(), reader.clone(), writer);

        let res = service_instances.remove_service_instance(
            ServiceId::new("alpha".into(), "abc".into()),
            "my_component".into(),
        );

        assert!(matches!(
            res,
            Err(ServiceRemoveInstanceError::NotRegistered)
        ));
    }

    #[test]
    fn test_remove_service_instance_accepted() {
        let (circuit, nodes) = build_circuit();

        let table = RoutingTable::default();
        let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
        let mut writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        writer
            .add_circuit(circuit.circuit_id().into(), circuit, nodes)
            .expect("Unable to add circuit");

        let id = ServiceId::new("alpha".into(), "abc".into());
        let mut service = reader
            .get_service(&id)
            .expect("Unable to get service")
            .expect("Missing service");
        service.set_peer_id("abc_network".into());
        writer
            .add_service(id.clone(), service)
            .expect("Unable to add service");

        let service_instances =
            RoutingTableServiceInstances::new("123".to_string(), reader.clone(), writer);

        let res = service_instances.remove_service_instance(id.clone(), "my_component".into());

        assert!(matches!(res, Ok(())));

        assert!(reader
            .get_service(&id)
            .expect("cannot check if it has the service")
            .expect("no service returned")
            .peer_id()
            .is_none());
    }

    fn build_circuit() -> (Circuit, Vec<CircuitNode>) {
        let node_123 = CircuitNode::new("123".to_string(), vec!["123.0.0.1:0".to_string()]);
        let node_345 = CircuitNode::new("345".to_string(), vec!["123.0.0.1:1".to_string()]);

        let service_abc = Service::new(
            "abc".to_string(),
            "test".to_string(),
            "123".to_string(),
            vec![],
        );
        let service_def = Service::new(
            "def".to_string(),
            "test".to_string(),
            "345".to_string(),
            vec![],
        );

        // Add circuit and service to splinter state
        let circuit = Circuit::new(
            "alpha".into(),
            vec![service_abc.clone(), service_def.clone()],
            vec!["123".into(), "345".into()],
        );

        (circuit, vec![node_123, node_345])
    }
}
