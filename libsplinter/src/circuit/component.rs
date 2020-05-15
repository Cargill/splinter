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

use crate::circuit::service::{Service, ServiceId, SplinterNode};
use crate::circuit::{ServiceDefinition, SplinterState};
use crate::service::network::{
    ServiceAddInstanceError, ServiceInstances, ServiceRemoveInstanceError,
};

/// A collection of service instances, backed by `SplinterState`.
pub struct SplinterStateServiceInstances {
    splinter_node: SplinterNode,
    state: SplinterState,
}

impl SplinterStateServiceInstances {
    /// Construct a new instance with the given local node information and an instance of splinter
    /// state.
    ///
    /// # Params
    ///
    /// - `splinter_node`: the local node information
    /// - `state`: an instance of splinter state, that will be used to store the service instances
    pub fn new(splinter_node: SplinterNode, state: SplinterState) -> Self {
        Self {
            splinter_node,
            state,
        }
    }
}

impl ServiceInstances for SplinterStateServiceInstances {
    fn add_service_instance(
        &self,
        service_id: ServiceId,
        component_id: String,
    ) -> Result<(), ServiceAddInstanceError> {
        let has_service = self.state.has_service(&service_id).map_err(|err| {
            ServiceAddInstanceError::InternalError {
                context: format!(
                    "unable to check if service {} is already registered",
                    service_id
                ),
                source: Some(Box::new(err)),
            }
        })?;

        if has_service {
            return Err(ServiceAddInstanceError::AlreadyRegistered);
        }

        let unique_id = service_id.clone();
        let (circuit_name, service_id) = service_id.into_parts();

        let circuit = self
            .state
            .circuit(&circuit_name)
            .map_err(|err| ServiceAddInstanceError::InternalError {
                context: format!("unable to load circuit information for {}", unique_id),
                source: Some(Box::new(err)),
            })?
            .ok_or(ServiceAddInstanceError::CircuitDoesNotExist)?;

        let service_def = if !service_id.starts_with("admin::") {
            circuit
                .roster()
                .iter()
                .find(|service| service.service_id == service_id)
                .cloned()
        } else {
            Some(
                ServiceDefinition::builder(service_id.clone(), "admin".into())
                    .with_allowed_nodes(vec![self.splinter_node.id().to_string()])
                    .build(),
            )
        };

        if let Some(service_def) = service_def {
            if !service_def
                .allowed_nodes
                .iter()
                .any(|node_id| node_id == self.splinter_node.id())
            {
                return Err(ServiceAddInstanceError::NotAllowed);
            }

            let service = Service::new(
                service_id.clone(),
                Some(component_id),
                self.splinter_node.clone(),
            );

            self.state.add_service(unique_id, service).map_err(|err| {
                ServiceAddInstanceError::InternalError {
                    context: format!("unable to add service {}", service_id),
                    source: Some(Box::new(err)),
                }
            })?;
        } else {
            return Err(ServiceAddInstanceError::NotInCircuit);
        }
        Ok(())
    }

    fn remove_service_instance(
        &self,
        service_id: ServiceId,
        _component_id: String,
    ) -> Result<(), ServiceRemoveInstanceError> {
        let has_service = !self.state.has_service(&service_id).map_err(|err| {
            ServiceRemoveInstanceError::InternalError {
                context: format!("unable to check if service {} is registered", service_id),
                source: Some(Box::new(err)),
            }
        })?;

        if has_service {
            return Err(ServiceRemoveInstanceError::NotRegistered);
        }

        self.state
            .remove_service(&service_id)
            .map(|_| ())
            .map_err(|err| ServiceRemoveInstanceError::InternalError {
                context: format!("unable to remove service {}", service_id),
                source: Some(Box::new(err)),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::circuit::directory::CircuitDirectory;
    use crate::circuit::{AuthorizationType, Circuit, DurabilityType, PersistenceType, RouteType};

    #[test]
    // Test that if the circuit does not exist, a ServiceAddInstanceError::CircuitDoesNotExist
    // error is returned.
    fn test_add_service_instance_circuit_does_not_exist() {
        let circuit_directory = CircuitDirectory::new();
        let state = SplinterState::new("memory".to_string(), circuit_directory);

        let splinter_node = SplinterNode::new("123".into(), vec!["tcp://127.0.0.1:0".into()]);
        let service_instances = SplinterStateServiceInstances::new(splinter_node, state);

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
        let circuit = build_circuit();

        let mut circuit_directory = CircuitDirectory::new();
        circuit_directory.add_circuit("alpha".into(), circuit);
        let state = SplinterState::new("memory".to_string(), circuit_directory);

        let splinter_node = SplinterNode::new("123".into(), vec!["tcp://127.0.0.1:0".into()]);
        let service_instances = SplinterStateServiceInstances::new(splinter_node, state);

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
        let circuit = build_circuit();

        let mut circuit_directory = CircuitDirectory::new();
        circuit_directory.add_circuit("alpha".into(), circuit);
        let state = SplinterState::new("memory".to_string(), circuit_directory);

        let splinter_node = SplinterNode::new("123".into(), vec!["tcp://127.0.0.1:0".into()]);
        let service = Service::new(
            "abc".to_string(),
            Some("abc_network".to_string()),
            splinter_node.clone(),
        );
        let id = ServiceId::new("alpha".into(), "abc".into());
        state.add_service(id.clone(), service).unwrap();

        let service_instances = SplinterStateServiceInstances::new(splinter_node, state);

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
        let circuit = build_circuit();

        let mut circuit_directory = CircuitDirectory::new();
        circuit_directory.add_circuit("alpha".to_string(), circuit);

        let state = SplinterState::new("memory".to_string(), circuit_directory);

        let splinter_node = SplinterNode::new("123".into(), vec!["tcp://127.0.0.1:0".into()]);
        let service_instances = SplinterStateServiceInstances::new(splinter_node, state.clone());

        let id = ServiceId::new("alpha".into(), "abc".into());
        let res = service_instances.add_service_instance(id.clone(), "my_component".into());

        assert!(matches!(res, Ok(())));
        assert!(state
            .has_service(&id)
            .expect("cannot check if it has the service"));
    }

    #[test]
    // Test that if the circuit is not registered, a ServiceRemoveInstanceError::NotRegistered
    // should be returned.
    fn test_remove_service_instance_not_registred() {
        let circuit = build_circuit();

        let mut circuit_directory = CircuitDirectory::new();
        circuit_directory.add_circuit("alpha".into(), circuit);
        let state = SplinterState::new("memory".to_string(), circuit_directory);

        let splinter_node = SplinterNode::new("123".into(), vec!["tcp://127.0.0.1:0".into()]);
        let service_instances = SplinterStateServiceInstances::new(splinter_node, state);

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
        let circuit = build_circuit();

        let mut circuit_directory = CircuitDirectory::new();
        circuit_directory.add_circuit("alpha".into(), circuit);
        let state = SplinterState::new("memory".to_string(), circuit_directory);

        let splinter_node = SplinterNode::new("123".into(), vec!["tcp://127.0.0.1:0".into()]);
        let service = Service::new(
            "abc".to_string(),
            Some("abc_network".to_string()),
            splinter_node.clone(),
        );
        let id = ServiceId::new("alpha".into(), "abc".into());
        state.add_service(id.clone(), service).unwrap();

        let service_instances = SplinterStateServiceInstances::new(splinter_node, state.clone());

        let res = service_instances.remove_service_instance(id.clone(), "my_component".into());

        assert!(matches!(res, Ok(())));

        assert!(!state
            .has_service(&id)
            .expect("cannot check if it has the service"));
    }

    fn build_circuit() -> Circuit {
        let service_abc = ServiceDefinition::builder("abc".into(), "test".into())
            .with_allowed_nodes(vec!["123".to_string()])
            .build();

        let service_def = ServiceDefinition::builder("def".into(), "test".into())
            .with_allowed_nodes(vec!["345".to_string()])
            .build();

        let circuit = Circuit::builder()
            .with_id("alpha".into())
            .with_auth(AuthorizationType::Trust)
            .with_members(vec!["123".into(), "345".into()])
            .with_roster(vec![service_abc, service_def])
            .with_persistence(PersistenceType::Any)
            .with_durability(DurabilityType::NoDurability)
            .with_routes(RouteType::Any)
            .with_circuit_management_type("service_connect_test_app".into())
            .build()
            .expect("Should have built a correct circuit");

        circuit
    }
}
