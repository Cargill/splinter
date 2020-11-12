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

//! Defines a YAML backed implementation of the `AdminServiceStore`. The goal of this
//! implementation is to support Splinter v0.4 YAML state files.
//!
//! The public interface includes the struct [`YamlAdminServiceStore`].
//!
//! [`YamlAdminServiceStore`]: struct.YamlAdminServiceStore.html

use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fs::{rename, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::{
    AdminServiceStore, AdminServiceStoreError, AuthorizationType, Circuit, CircuitBuilder,
    CircuitNode, CircuitNodeBuilder, CircuitPredicate, CircuitProposal, CircuitProposalBuilder,
    DurabilityType, PersistenceType, ProposalType, ProposedCircuit, ProposedCircuitBuilder,
    ProposedNode, ProposedNodeBuilder, ProposedService, ProposedServiceBuilder, RouteType, Service,
    ServiceBuilder, ServiceId, Vote, VoteRecord, VoteRecordBuilder,
};

use crate::error::{
    ConstraintViolationError, ConstraintViolationType, InternalError, InvalidStateError,
};
use crate::hex::{parse_hex, to_hex};

/// A YAML backed implementation of the `AdminServiceStore`
pub struct YamlAdminServiceStore {
    circuit_file_path: String,
    proposal_file_path: String,
    state: Arc<Mutex<YamlState>>,
}

impl YamlAdminServiceStore {
    /// Creates a new `YamlAdminServiceStore`. If the file paths provided exist, the existing state
    /// will be cached in the store. If the files do not exist, they will be created with empty
    /// state.
    ///
    /// # Arguments
    ///
    ///  * `circuit_file_path` - The path to file that contains circuit state
    ///  * `proposal_file_path` - The path to file that contains circuit proposal state
    ///
    /// Returns an error if the file paths cannot be read from or written to
    pub fn new(
        circuit_file_path: String,
        proposal_file_path: String,
    ) -> Result<Self, AdminServiceStoreError> {
        let mut store = YamlAdminServiceStore {
            circuit_file_path: circuit_file_path.to_string(),
            proposal_file_path: proposal_file_path.to_string(),
            state: Arc::new(Mutex::new(YamlState::default())),
        };

        let circuit_file_path_buf = PathBuf::from(circuit_file_path);
        let proposal_file_path_buf = PathBuf::from(proposal_file_path);

        // If file already exists, read it; otherwise initialize it.
        if circuit_file_path_buf.is_file() && proposal_file_path_buf.is_file() {
            store.read_state()?;
        } else if circuit_file_path_buf.is_file() {
            // read circuit
            store.read_circuit_state()?;
            // write proposals
            store.write_proposal_state()?;
        } else if proposal_file_path_buf.is_file() {
            // write circuit
            store.write_circuit_state()?;
            // read proposals
            store.read_proposal_state()?;
        } else {
            // write all empty state
            store.write_state()?;
        }

        Ok(store)
    }

    /// Read circuit state from the circuit file path and cache the contents in the store
    fn read_circuit_state(&mut self) -> Result<(), AdminServiceStoreError> {
        let circuit_file = File::open(&self.circuit_file_path).map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                "Failed to open YAML circuit state file".to_string(),
            ))
        })?;

        let yaml_state_circuits: YamlCircuitState = serde_yaml::from_reader(&circuit_file)
            .map_err(|err| {
                AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                    Box::new(err),
                    "Failed to read YAML circuit state file".to_string(),
                ))
            })?;

        let yaml_state = CircuitState::try_from(yaml_state_circuits)
            .map_err(AdminServiceStoreError::InvalidStateError)?;

        let mut state = self.state.lock().map_err(|_| {
            AdminServiceStoreError::InternalError(InternalError::with_message(
                "YAML admin service store's internal lock poisoned".to_string(),
            ))
        })?;

        for (circuit_id, circuit) in yaml_state.circuits.iter() {
            for service in circuit.roster() {
                let service_id =
                    ServiceId::new(service.service_id().to_string(), circuit_id.to_string());

                state.service_directory.insert(service_id, service.clone());
            }
        }

        state.circuit_state = yaml_state;
        Ok(())
    }

    /// Read circuit proposal state from the proposal file path and cache the contents in the
    /// store
    fn read_proposal_state(&mut self) -> Result<(), AdminServiceStoreError> {
        let proposal_file = File::open(&self.proposal_file_path).map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                "Failed to open YAML proposal state file".to_string(),
            ))
        })?;

        let yaml_proposals_state: YamlProposalState = serde_yaml::from_reader(&proposal_file)
            .map_err(|err| {
                AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                    Box::new(err),
                    "Failed to read YAML proposal state file".to_string(),
                ))
            })?;

        let proposals_state = ProposalState::try_from(yaml_proposals_state)
            .map_err(AdminServiceStoreError::InvalidStateError)?;

        let mut state = self.state.lock().map_err(|_| {
            AdminServiceStoreError::InternalError(InternalError::with_message(
                "YAML admin service store's internal lock poisoned".to_string(),
            ))
        })?;

        state.proposal_state = proposals_state;
        Ok(())
    }

    /// Read circuit state from the circuit file path and cache the contents in the store and then
    /// read circuit proposal state from the proposal file path and cache the contents in the
    /// store
    fn read_state(&mut self) -> Result<(), AdminServiceStoreError> {
        let circuit_file = File::open(&self.circuit_file_path).map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                "Failed to open YAML circuit state file".to_string(),
            ))
        })?;

        let yaml_state_circuits: YamlCircuitState = serde_yaml::from_reader(&circuit_file)
            .map_err(|err| {
                AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                    Box::new(err),
                    "Failed to read YAML circuit state file".to_string(),
                ))
            })?;

        let yaml_state = CircuitState::try_from(yaml_state_circuits)
            .map_err(AdminServiceStoreError::InvalidStateError)?;

        let proposal_file = File::open(&self.proposal_file_path).map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                "Failed to open YAML proposal state file".to_string(),
            ))
        })?;

        let yaml_proposals_state: YamlProposalState = serde_yaml::from_reader(&proposal_file)
            .map_err(|err| {
                AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                    Box::new(err),
                    "Failed to read YAML proposal state file".to_string(),
                ))
            })?;

        let proposals_state = ProposalState::try_from(yaml_proposals_state)
            .map_err(AdminServiceStoreError::InvalidStateError)?;

        let mut state = self.state.lock().map_err(|_| {
            AdminServiceStoreError::InternalError(InternalError::with_message(
                "YAML admin service store's internal lock poisoned".to_string(),
            ))
        })?;

        for (circuit_id, circuit) in yaml_state.circuits.iter() {
            for service in circuit.roster() {
                let service_id =
                    ServiceId::new(service.service_id().to_string(), circuit_id.to_string());

                state.service_directory.insert(service_id, service.clone());
            }
        }

        state.circuit_state = yaml_state;
        state.proposal_state = proposals_state;

        Ok(())
    }

    /// Write the current circuit state to file at the circuit file path
    fn write_circuit_state(&self) -> Result<(), AdminServiceStoreError> {
        let state = self.state.lock().map_err(|_| {
            AdminServiceStoreError::InternalError(InternalError::with_message(
                "YAML admin service store's internal lock poisoned".to_string(),
            ))
        })?;

        let circuit_output = serde_yaml::to_vec(&YamlCircuitState::from(
            state.circuit_state.clone(),
        ))
        .map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                "Failed to write circuit state to YAML".to_string(),
            ))
        })?;

        // write state to a temporary file to avoid state corruption if an IO error occurs during
        // write
        let temp_circuit_file = format!("{}.temp", self.circuit_file_path);
        let mut circuit_file = File::create(&temp_circuit_file).map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                format!(
                    "Failed to open YAML circuit state file '{}'",
                    temp_circuit_file
                ),
            ))
        })?;

        circuit_file.write_all(&circuit_output).map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                format!(
                    "Failed to write to YAML circuit state file '{}'",
                    temp_circuit_file
                ),
            ))
        })?;

        // Append newline to file
        writeln!(circuit_file).map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                format!(
                    "Failed to write to YAML circuit file '{}'",
                    temp_circuit_file
                ),
            ))
        })?;

        // rename temp file to circuit state filename
        rename(&temp_circuit_file, &self.circuit_file_path).map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                format!(
                    "Failed to rename temp circuit state file to final location'{}'",
                    temp_circuit_file
                ),
            ))
        })?;

        Ok(())
    }

    /// Write the current circuit proposal state to file at the proposal file path
    fn write_proposal_state(&self) -> Result<(), AdminServiceStoreError> {
        let state = self.state.lock().map_err(|_| {
            AdminServiceStoreError::InternalError(InternalError::with_message(
                "YAML admin service store's internal lock poisoned".to_string(),
            ))
        })?;

        let proposal_output = serde_yaml::to_vec(&YamlProposalState::from(
            state.proposal_state.clone(),
        ))
        .map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                "Failed to write proposals state to YAML".to_string(),
            ))
        })?;

        let temp_proposal_file = format!("{}.temp", self.proposal_file_path);
        let mut proposal_file = File::create(&temp_proposal_file).map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                format!(
                    "Failed to open YAML proposal state file '{}'",
                    temp_proposal_file
                ),
            ))
        })?;

        proposal_file.write_all(&proposal_output).map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                format!(
                    "Failed to write YAML proposal state file '{}'",
                    temp_proposal_file
                ),
            ))
        })?;

        // Append newline to file
        writeln!(proposal_file).map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                format!(
                    "Failed to write to YAML proposal file '{}'",
                    temp_proposal_file
                ),
            ))
        })?;

        // rename temp file to proposal state filename
        rename(&temp_proposal_file, &self.proposal_file_path).map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                format!(
                    "Failed to rename temp proposal state file to final location '{}'",
                    temp_proposal_file
                ),
            ))
        })?;

        Ok(())
    }

    /// Write the current circuit state to file at the circuit file path and then write the current
    /// proposal state to the file at the proposal file path
    fn write_state(&self) -> Result<(), AdminServiceStoreError> {
        let state = self.state.lock().map_err(|_| {
            AdminServiceStoreError::InternalError(InternalError::with_message(
                "YAML admin service store's internal lock poisoned".to_string(),
            ))
        })?;

        let circuit_output = serde_yaml::to_vec(&YamlCircuitState::from(
            state.circuit_state.clone(),
        ))
        .map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                "Failed to write circuit state to YAML".to_string(),
            ))
        })?;

        // write state to a temporary file to avoid state corruption if an IO error occurs during
        // write
        let temp_circuit_file = format!("{}.temp", self.circuit_file_path);
        let mut circuit_file = File::create(&temp_circuit_file).map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                format!(
                    "Failed to open YAML circuit state file '{}'",
                    temp_circuit_file
                ),
            ))
        })?;

        circuit_file.write_all(&circuit_output).map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                format!(
                    "Failed to write to YAML circuit state file '{}'",
                    temp_circuit_file
                ),
            ))
        })?;

        // Append newline to file
        writeln!(circuit_file).map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                format!(
                    "Failed to write to YAML circuit file '{}'",
                    temp_circuit_file
                ),
            ))
        })?;

        // rename temp file to circuit state filename
        rename(&temp_circuit_file, &self.circuit_file_path).map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                format!(
                    "Failed to rename temp circuit state file to final location '{}'",
                    temp_circuit_file
                ),
            ))
        })?;

        let proposal_output = serde_yaml::to_vec(&YamlProposalState::from(
            state.proposal_state.clone(),
        ))
        .map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                "Failed to write proposals state to YAML".to_string(),
            ))
        })?;

        // write state to a temporary file to avoid state corruption if an IO error occurs during
        // write
        let temp_proposal_file = format!("{}.temp", self.proposal_file_path);
        let mut proposal_file = File::create(&temp_proposal_file).map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                format!(
                    "Failed to open YAML proposal state file '{}'",
                    temp_proposal_file
                ),
            ))
        })?;

        proposal_file.write_all(&proposal_output).map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                format!(
                    "Failed to write to YAML proposal state file '{}'",
                    temp_proposal_file
                ),
            ))
        })?;

        // Append newline to file
        writeln!(proposal_file).map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                format!(
                    "Failed to write to YAML proposal file '{}'",
                    self.proposal_file_path
                ),
            ))
        })?;

        // rename temp file to proposal state filename
        rename(&temp_proposal_file, &self.proposal_file_path).map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                format!(
                    "Failed to rename temp proposal state file to final location '{}'",
                    temp_proposal_file
                ),
            ))
        })?;

        Ok(())
    }
}

/// Defines methods for CRUD operations and fetching and listing circuits, proposals, nodes and
/// services from a YAML file backend
impl AdminServiceStore for YamlAdminServiceStore {
    /// Adds a circuit proposal to the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `proposal` - The proposal to be added
    ///
    ///  Returns an error if a `CircuitProposal` with the same ID already exists
    fn add_proposal(&self, proposal: CircuitProposal) -> Result<(), AdminServiceStoreError> {
        {
            let mut state = self.state.lock().map_err(|_| {
                AdminServiceStoreError::InternalError(InternalError::with_message(
                    "YAML admin service store's internal lock was poisoned".to_string(),
                ))
            })?;

            if state
                .proposal_state
                .proposals
                .contains_key(proposal.circuit_id())
            {
                return Err(AdminServiceStoreError::ConstraintViolationError(
                    ConstraintViolationError::with_violation_type(ConstraintViolationType::Unique),
                ));
            } else {
                state
                    .proposal_state
                    .proposals
                    .insert(proposal.circuit_id().to_string(), proposal);
            }
        }

        self.write_proposal_state().map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                "Unable to write proposal state yaml file".to_string(),
            ))
        })
    }

    /// Updates a circuit proposal in the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `proposal` - The proposal with the updated information
    ///
    ///  Returns an error if a `CircuitProposal` with the same ID does not exist
    fn update_proposal(&self, proposal: CircuitProposal) -> Result<(), AdminServiceStoreError> {
        {
            let mut state = self.state.lock().map_err(|_| {
                AdminServiceStoreError::InternalError(InternalError::with_message(
                    "YAML admin service store's internal lock was poisoned".to_string(),
                ))
            })?;

            if state
                .proposal_state
                .proposals
                .contains_key(proposal.circuit_id())
            {
                state
                    .proposal_state
                    .proposals
                    .insert(proposal.circuit_id().to_string(), proposal);
            } else {
                return Err(AdminServiceStoreError::InvalidStateError(
                    InvalidStateError::with_message(format!(
                        "A proposal with ID {} does not exist",
                        proposal.circuit_id()
                    )),
                ));
            }
        }

        self.write_proposal_state().map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                "Unable to write proposal state yaml file".to_string(),
            ))
        })
    }

    /// Removes a circuit proposal from the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `proposal_id` - The unique ID of the circuit proposal to be removed
    ///
    ///  Returns an error if a `CircuitProposal` with specified ID does not exist
    fn remove_proposal(&self, proposal_id: &str) -> Result<(), AdminServiceStoreError> {
        {
            let mut state = self.state.lock().map_err(|_| {
                AdminServiceStoreError::InternalError(InternalError::with_message(
                    "YAML admin service store's internal lock was poisoned".to_string(),
                ))
            })?;

            if state.proposal_state.proposals.contains_key(proposal_id) {
                state.proposal_state.proposals.remove(proposal_id);
            } else {
                return Err(AdminServiceStoreError::InvalidStateError(
                    InvalidStateError::with_message(format!(
                        "A proposal with ID {} does not exist",
                        proposal_id
                    )),
                ));
            }
        }

        self.write_proposal_state().map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                "Unable to write proposal state yaml file".to_string(),
            ))
        })
    }

    /// Fetches a circuit proposal from the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `proposal_id` - The unique ID of the circuit proposal to be returned
    fn get_proposal(
        &self,
        proposal_id: &str,
    ) -> Result<Option<CircuitProposal>, AdminServiceStoreError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| {
                AdminServiceStoreError::InternalError(InternalError::with_message(
                    "YAML admin service store's internal lock was poisoned".to_string(),
                ))
            })?
            .proposal_state
            .proposals
            .get(proposal_id)
            .cloned())
    }

    /// List circuit proposals from the underlying storage
    ///
    /// The proposals returned can be filtered by provided CircuitPredicate. This enables
    /// filtering by management type and members.
    fn list_proposals(
        &self,
        predicates: &[CircuitPredicate],
    ) -> Result<Box<dyn ExactSizeIterator<Item = CircuitProposal>>, AdminServiceStoreError> {
        let mut proposals: Vec<CircuitProposal> = self
            .state
            .lock()
            .map_err(|_| {
                AdminServiceStoreError::InternalError(InternalError::with_message(
                    "YAML admin service store's internal lock was poisoned".to_string(),
                ))
            })?
            .proposal_state
            .proposals
            .iter()
            .map(|(_, proposal)| proposal.clone())
            .collect::<Vec<CircuitProposal>>();

        proposals.retain(|proposal| {
            predicates
                .iter()
                .all(|predicate| predicate.apply_to_proposals(proposal))
        });

        Ok(Box::new(proposals.into_iter()))
    }

    /// Adds a circuit to the underlying storage. Also includes the associated Services and
    /// Nodes
    ///
    /// # Arguments
    ///
    ///  * `circuit` - The circuit to be added to state
    ///  * `nodes` - A list of nodes that represent the circuit's members
    ///
    ///  Returns an error if a `Circuit` with the same ID already exists
    fn add_circuit(
        &self,
        circuit: Circuit,
        nodes: Vec<CircuitNode>,
    ) -> Result<(), AdminServiceStoreError> {
        {
            let mut state = self.state.lock().map_err(|_| {
                AdminServiceStoreError::InternalError(InternalError::with_message(
                    "YAML admin service store's internal lock was poisoned".to_string(),
                ))
            })?;

            if state
                .circuit_state
                .circuits
                .contains_key(circuit.circuit_id())
            {
                return Err(AdminServiceStoreError::ConstraintViolationError(
                    ConstraintViolationError::with_violation_type(ConstraintViolationType::Unique),
                ));
            } else {
                for service in circuit.roster() {
                    let service_id = ServiceId::new(
                        service.service_id().to_string(),
                        circuit.circuit_id().to_string(),
                    );

                    state.service_directory.insert(service_id, service.clone());
                }

                for node in nodes.into_iter() {
                    if !state.circuit_state.nodes.contains_key(node.node_id()) {
                        state
                            .circuit_state
                            .nodes
                            .insert(node.node_id().to_string(), node);
                    }
                }

                state
                    .circuit_state
                    .circuits
                    .insert(circuit.circuit_id().to_string(), circuit);
            }
        }

        self.write_circuit_state().map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                "Unable to write circuit state yaml file".to_string(),
            ))
        })
    }

    /// Updates a circuit in the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `circuit` - The circuit with the updated information
    ///
    ///  Returns an error if a `CircuitProposal` with the same ID does not exist
    fn update_circuit(&self, circuit: Circuit) -> Result<(), AdminServiceStoreError> {
        {
            let mut state = self.state.lock().map_err(|_| {
                AdminServiceStoreError::InternalError(InternalError::with_message(
                    "YAML admin service store's internal lock was poisoned".to_string(),
                ))
            })?;

            if state
                .circuit_state
                .circuits
                .contains_key(circuit.circuit_id())
            {
                state
                    .circuit_state
                    .circuits
                    .insert(circuit.circuit_id().to_string(), circuit);
            } else {
                return Err(AdminServiceStoreError::InvalidStateError(
                    InvalidStateError::with_message(format!(
                        "A circuit with ID {} does not exist",
                        circuit.circuit_id()
                    )),
                ));
            }
        }

        self.write_circuit_state().map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                "Unable to write circuit state yaml file".to_string(),
            ))
        })
    }

    /// Removes a circuit from the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `circuit_id` - The unique ID of the circuit to be removed
    ///
    ///  Returns an error if a `Circuit` with the specified ID does not exist
    fn remove_circuit(&self, circuit_id: &str) -> Result<(), AdminServiceStoreError> {
        {
            let mut state = self.state.lock().map_err(|_| {
                AdminServiceStoreError::InternalError(InternalError::with_message(
                    "YAML admin service store's internal lock was poisoned".to_string(),
                ))
            })?;
            if state.circuit_state.circuits.contains_key(circuit_id) {
                let circuit = state.circuit_state.circuits.remove(circuit_id);
                if let Some(circuit) = circuit {
                    for service in circuit.roster() {
                        let service_id = ServiceId::new(
                            service.service_id().to_string(),
                            circuit_id.to_string(),
                        );
                        state.service_directory.remove(&service_id);
                    }
                }
            } else {
                return Err(AdminServiceStoreError::InvalidStateError(
                    InvalidStateError::with_message(format!(
                        "A circuit with ID {} does not exist",
                        circuit_id
                    )),
                ));
            }
        }

        self.write_circuit_state().map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                "Unable to write circuit state yaml file".to_string(),
            ))
        })
    }

    /// Fetches a circuit from the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `circuit_id` - The unique ID of the circuit to be returned
    fn get_circuit(&self, circuit_id: &str) -> Result<Option<Circuit>, AdminServiceStoreError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| {
                AdminServiceStoreError::InternalError(InternalError::with_message(
                    "YAML admin service store's internal lock was poisoned".to_string(),
                ))
            })?
            .circuit_state
            .circuits
            .get(circuit_id)
            .cloned())
    }

    /// List all circuits from the underlying storage
    ///
    /// The proposals returned can be filtered by provided CircuitPredicate. This enables
    /// filtering by management type and members.
    fn list_circuits(
        &self,
        predicates: &[CircuitPredicate],
    ) -> Result<Box<dyn ExactSizeIterator<Item = Circuit>>, AdminServiceStoreError> {
        let mut circuits: Vec<Circuit> = self
            .state
            .lock()
            .map_err(|_| {
                AdminServiceStoreError::InternalError(InternalError::with_message(
                    "YAML admin service store's internal lock was poisoned".to_string(),
                ))
            })?
            .circuit_state
            .circuits
            .iter()
            .map(|(_, circuit)| circuit.clone())
            .collect();

        circuits.retain(|circuit| {
            predicates
                .iter()
                .all(|predicate| predicate.apply_to_circuit(circuit))
        });

        Ok(Box::new(circuits.into_iter()))
    }

    /// Adds a circuit to the underlying storage based on the proposal that is already in state..
    /// Also includes the associated Services and Nodes. The associated circuit proposal for
    /// the circuit ID is also removed
    ///
    /// # Arguments
    ///
    ///  * `circuit_id` - The ID of the circuit proposal that should be converted to a circuit
    fn upgrade_proposal_to_circuit(&self, circuit_id: &str) -> Result<(), AdminServiceStoreError> {
        {
            let mut state = self.state.lock().map_err(|_| {
                AdminServiceStoreError::InternalError(InternalError::with_message(
                    "YAML admin service store's internal lock was poisoned".to_string(),
                ))
            })?;

            if let Some(proposal) = state.proposal_state.proposals.remove(circuit_id) {
                let nodes = proposal.circuit().members().to_vec();
                let services = proposal.circuit().roster().to_vec();

                let circuit = Circuit::from(proposal.circuit().clone());
                state
                    .circuit_state
                    .circuits
                    .insert(circuit.circuit_id().to_string(), circuit);

                for service in services.into_iter() {
                    let service_id =
                        ServiceId::new(service.service_id().to_string(), circuit_id.to_string());

                    state
                        .service_directory
                        .insert(service_id, Service::from(service));
                }

                for node in nodes.into_iter() {
                    if !state.circuit_state.nodes.contains_key(node.node_id()) {
                        state
                            .circuit_state
                            .nodes
                            .insert(node.node_id().to_string(), CircuitNode::from(node));
                    }
                }
            } else {
                return Err(AdminServiceStoreError::InvalidStateError(
                    InvalidStateError::with_message(format!(
                        "A circuit proposal with ID {} does not exist",
                        circuit_id
                    )),
                ));
            }
        }

        self.write_state().map_err(|err| {
            AdminServiceStoreError::InternalError(InternalError::from_source_with_prefix(
                Box::new(err),
                "Unable to write circuit state yaml file".to_string(),
            ))
        })
    }

    /// Fetches a node from the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `node_id` - The unique ID of the node to be returned
    fn get_node(&self, node_id: &str) -> Result<Option<CircuitNode>, AdminServiceStoreError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| {
                AdminServiceStoreError::InternalError(InternalError::with_message(
                    "YAML admin service store's internal lock was poisoned".to_string(),
                ))
            })?
            .circuit_state
            .nodes
            .get(node_id)
            .cloned())
    }

    /// List all nodes from the underlying storage
    fn list_nodes(
        &self,
    ) -> Result<Box<dyn ExactSizeIterator<Item = CircuitNode>>, AdminServiceStoreError> {
        let nodes: Vec<CircuitNode> = self
            .state
            .lock()
            .map_err(|_| {
                AdminServiceStoreError::InternalError(InternalError::with_message(
                    "YAML admin service store's internal lock was poisoned".to_string(),
                ))
            })?
            .circuit_state
            .nodes
            .iter()
            .map(|(_, node)| node.clone())
            .collect();

        Ok(Box::new(nodes.into_iter()))
    }

    /// Fetches a service from the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `service_id` - The `ServiceId` of a service made up of the circuit ID and service ID
    fn get_service(
        &self,
        service_id: &ServiceId,
    ) -> Result<Option<Service>, AdminServiceStoreError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| {
                AdminServiceStoreError::InternalError(InternalError::with_message(
                    "YAML admin service store's internal lock was poisoned".to_string(),
                ))
            })?
            .service_directory
            .get(service_id)
            .cloned())
    }

    /// List all services in a specific circuit from the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `circuit_id` - The unique ID of the circuit the services belong to
    fn list_services(
        &self,
        circuit_id: &str,
    ) -> Result<Box<dyn ExactSizeIterator<Item = Service>>, AdminServiceStoreError> {
        let services: Vec<Service> =
            self.state
                .lock()
                .map_err(|_| {
                    AdminServiceStoreError::InternalError(InternalError::with_message(
                        "YAML admin service store's internal lock was poisoned".to_string(),
                    ))
                })?
                .circuit_state
                .circuits
                .get(circuit_id)
                .ok_or_else(|| {
                    AdminServiceStoreError::InvalidStateError(InvalidStateError::with_message(
                        format!("A circuit with ID {} does not exist", circuit_id),
                    ))
                })?
                .roster()
                .to_vec();

        Ok(Box::new(services.into_iter()))
    }
}

/// YAML file specific circuit definition. This circuit definition in the 0.4v YAML stores service
/// arguments in a map format, which differs from the definition defined in the AdminServiceStore.
/// To handle this, circuit needs to be converted to the correct format during read/write
/// operations.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
struct YamlCircuit {
    id: String,
    roster: Vec<YamlService>,
    members: Vec<String>,
    auth: YamlAuthorizationType,
    persistence: YamlPersistenceType,
    durability: YamlDurabilityType,
    routes: YamlRouteType,
    circuit_management_type: String,
}

impl TryFrom<YamlCircuit> for Circuit {
    type Error = InvalidStateError;

    fn try_from(circuit: YamlCircuit) -> Result<Self, Self::Error> {
        CircuitBuilder::new()
            .with_circuit_id(&circuit.id)
            .with_roster(
                &circuit
                    .roster
                    .into_iter()
                    .map(Service::try_from)
                    .collect::<Result<Vec<Service>, InvalidStateError>>()?,
            )
            .with_members(&circuit.members)
            .with_authorization_type(&AuthorizationType::from(circuit.auth))
            .with_persistence(&PersistenceType::from(circuit.persistence))
            .with_durability(&DurabilityType::from(circuit.durability))
            .with_routes(&RouteType::from(circuit.routes))
            .with_circuit_management_type(&circuit.circuit_management_type)
            .build()
    }
}

impl From<Circuit> for YamlCircuit {
    fn from(circuit: Circuit) -> Self {
        YamlCircuit {
            id: circuit.circuit_id().into(),
            roster: circuit
                .roster()
                .iter()
                .map(|service| YamlService::from(service.clone()))
                .collect(),
            members: circuit.members().to_vec(),
            auth: circuit.authorization_type().clone().into(),
            persistence: circuit.persistence().clone().into(),
            durability: circuit.durability().clone().into(),
            routes: circuit.routes().clone().into(),
            circuit_management_type: circuit.circuit_management_type().into(),
        }
    }
}

/// YAML file specific service definition. This service definition in the 0.4v YAML stores
/// arguments in a map format, which differs from the definition defined in the AdminServiceStore.
/// To handle this, service needs to be converted to the correct format during read/write
/// operations.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
struct YamlService {
    service_id: String,
    service_type: String,
    allowed_nodes: Vec<String>,
    arguments: BTreeMap<String, String>,
}

impl TryFrom<YamlService> for Service {
    type Error = InvalidStateError;

    fn try_from(service: YamlService) -> Result<Self, Self::Error> {
        ServiceBuilder::new()
            .with_service_id(&service.service_id)
            .with_service_type(&service.service_type)
            .with_node_id(&service.allowed_nodes.get(0).ok_or_else(|| {
                InvalidStateError::with_message("Must contain 1 node ID".to_string())
            })?)
            .with_arguments(
                &service
                    .arguments
                    .iter()
                    .map(|(key, value)| (key.to_string(), value.to_string()))
                    .collect::<Vec<(String, String)>>(),
            )
            .build()
    }
}

impl From<Service> for YamlService {
    fn from(service: Service) -> Self {
        YamlService {
            service_id: service.service_id().into(),
            service_type: service.service_type().into(),
            allowed_nodes: vec![service.node_id().into()],
            arguments: service
                .arguments()
                .iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        }
    }
}

/// YAML file specific state definition that can be read and written to the circuit YAML state file
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
struct YamlCircuitState {
    nodes: BTreeMap<String, YamlCircuitNode>,
    circuits: BTreeMap<String, YamlCircuit>,
}

impl TryFrom<YamlCircuitState> for CircuitState {
    type Error = InvalidStateError;

    fn try_from(state: YamlCircuitState) -> Result<Self, Self::Error> {
        Ok(CircuitState {
            nodes: state
                .nodes
                .into_iter()
                .map(|(id, node)| CircuitNode::try_from(node).map(|node| (id, node)))
                .collect::<Result<BTreeMap<String, CircuitNode>, InvalidStateError>>()?,
            circuits: state
                .circuits
                .into_iter()
                .map(|(id, circuit)| match Circuit::try_from(circuit) {
                    Ok(circuit) => Ok((id, circuit)),
                    Err(err) => Err(err),
                })
                .collect::<Result<BTreeMap<String, Circuit>, InvalidStateError>>()?,
        })
    }
}

impl From<CircuitState> for YamlCircuitState {
    fn from(state: CircuitState) -> Self {
        YamlCircuitState {
            nodes: state
                .nodes
                .into_iter()
                .map(|(id, node)| (id, node.into()))
                .collect::<BTreeMap<String, YamlCircuitNode>>(),
            circuits: state
                .circuits
                .into_iter()
                .map(|(id, circuit)| (id, YamlCircuit::from(circuit)))
                .collect(),
        }
    }
}

/// The circuit state that is cached by the YAML admin service store and used to respond to fetch
/// requests
#[derive(Debug, Clone, PartialEq, Default)]
struct CircuitState {
    nodes: BTreeMap<String, CircuitNode>,
    circuits: BTreeMap<String, Circuit>,
}

/// YAML file specific proposal definition. The YAML state requires that the requester public key
/// is converted to a hex string. To handle this, proposals needs to be converted to the correct
/// format during read/write operations.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct YamlCircuitProposal {
    proposal_type: YamlProposalType,
    circuit_id: String,
    circuit_hash: String,
    circuit: YamlProposedCircuit,
    votes: Vec<YamlVoteRecord>,
    requester: String,
    requester_node_id: String,
}

impl From<ProposalState> for YamlProposalState {
    fn from(state: ProposalState) -> Self {
        YamlProposalState {
            proposals: state
                .proposals
                .into_iter()
                .map(|(id, proposal)| (id, YamlCircuitProposal::from(proposal)))
                .collect(),
        }
    }
}

impl TryFrom<YamlProposalState> for ProposalState {
    type Error = InvalidStateError;

    fn try_from(state: YamlProposalState) -> Result<Self, Self::Error> {
        Ok(ProposalState {
            proposals: state
                .proposals
                .into_iter()
                .map(|(id, proposal)| match CircuitProposal::try_from(proposal) {
                    Ok(proposal) => Ok((id, proposal)),
                    Err(err) => Err(err),
                })
                .collect::<Result<BTreeMap<String, CircuitProposal>, InvalidStateError>>()?,
        })
    }
}

impl TryFrom<YamlCircuitProposal> for CircuitProposal {
    type Error = InvalidStateError;

    fn try_from(proposal: YamlCircuitProposal) -> Result<Self, Self::Error> {
        CircuitProposalBuilder::new()
            .with_circuit_id(&proposal.circuit_id)
            .with_proposal_type(&ProposalType::from(proposal.proposal_type))
            .with_circuit_hash(&proposal.circuit_hash)
            .with_circuit(&ProposedCircuit::try_from(proposal.circuit)?)
            .with_votes(
                &proposal
                    .votes
                    .into_iter()
                    .map(VoteRecord::try_from)
                    .collect::<Result<Vec<VoteRecord>, InvalidStateError>>()?,
            )
            .with_requester(&parse_hex(&proposal.requester).map_err(|_| {
                InvalidStateError::with_message("Requester public key is not valid hex".to_string())
            })?)
            .with_requester_node_id(&proposal.requester_node_id)
            .build()
    }
}

impl From<CircuitProposal> for YamlCircuitProposal {
    fn from(proposal: CircuitProposal) -> Self {
        YamlCircuitProposal {
            circuit_id: proposal.circuit_id().into(),
            proposal_type: proposal.proposal_type().clone().into(),
            circuit_hash: proposal.circuit_hash().into(),
            circuit: YamlProposedCircuit::from(proposal.circuit().clone()),
            votes: proposal
                .votes()
                .iter()
                .map(|vote| YamlVoteRecord::from(vote.clone()))
                .collect(),
            requester: to_hex(proposal.requester()),
            requester_node_id: proposal.requester_node_id().into(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum YamlProposalType {
    Create,
    UpdateRoster,
    AddNode,
    RemoveNode,
    Destroy,
}

impl From<YamlProposalType> for ProposalType {
    fn from(proposal_type: YamlProposalType) -> Self {
        match proposal_type {
            YamlProposalType::Create => ProposalType::Create,
            YamlProposalType::UpdateRoster => ProposalType::UpdateRoster,
            YamlProposalType::AddNode => ProposalType::AddNode,
            YamlProposalType::RemoveNode => ProposalType::RemoveNode,
            YamlProposalType::Destroy => ProposalType::Destroy,
        }
    }
}

impl From<ProposalType> for YamlProposalType {
    fn from(proposal_type: ProposalType) -> Self {
        match proposal_type {
            ProposalType::Create => YamlProposalType::Create,
            ProposalType::UpdateRoster => YamlProposalType::UpdateRoster,
            ProposalType::AddNode => YamlProposalType::AddNode,
            ProposalType::RemoveNode => YamlProposalType::RemoveNode,
            ProposalType::Destroy => YamlProposalType::Destroy,
        }
    }
}

/// YAML file specific vote record definition. The YAML state requires that the vote public key
/// is converted to a hex string. To handle this, proposals needs to be converted to the correct
/// format during read/write operations.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct YamlVoteRecord {
    public_key: String,
    vote: YamlVote,
    voter_node_id: String,
}

impl TryFrom<YamlVoteRecord> for VoteRecord {
    type Error = InvalidStateError;

    fn try_from(vote: YamlVoteRecord) -> Result<Self, Self::Error> {
        VoteRecordBuilder::new()
            .with_public_key(&parse_hex(&vote.public_key).map_err(|_| {
                InvalidStateError::with_message("Requester public key is not valid hex".to_string())
            })?)
            .with_vote(&Vote::from(vote.vote))
            .with_voter_node_id(&vote.voter_node_id)
            .build()
    }
}

impl From<VoteRecord> for YamlVoteRecord {
    fn from(vote: VoteRecord) -> Self {
        YamlVoteRecord {
            public_key: to_hex(vote.public_key()),
            vote: vote.vote().clone().into(),
            voter_node_id: vote.voter_node_id().into(),
        }
    }
}

/// YAML file specific proposed circuit definition. In the YAML format the application metadata
/// needs to be converted into a hex string. To handle this, circuit needs to be converted to the
/// correct format during read/write operations.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
struct YamlProposedCircuit {
    circuit_id: String,
    roster: Vec<YamlProposedService>,
    members: Vec<YamlProposedNode>,
    authorization_type: YamlAuthorizationType,
    persistence: YamlPersistenceType,
    durability: YamlDurabilityType,
    routes: YamlRouteType,
    circuit_management_type: String,
    application_metadata: String,
    comments: String,
}

impl TryFrom<YamlProposedCircuit> for ProposedCircuit {
    type Error = InvalidStateError;

    fn try_from(circuit: YamlProposedCircuit) -> Result<Self, Self::Error> {
        ProposedCircuitBuilder::new()
            .with_circuit_id(&circuit.circuit_id)
            .with_roster(
                &circuit
                    .roster
                    .into_iter()
                    .map(ProposedService::try_from)
                    .collect::<Result<Vec<ProposedService>, InvalidStateError>>()?,
            )
            .with_members(
                &circuit
                    .members
                    .into_iter()
                    .map(ProposedNode::try_from)
                    .collect::<Result<Vec<ProposedNode>, InvalidStateError>>()?,
            )
            .with_authorization_type(&AuthorizationType::from(circuit.authorization_type))
            .with_persistence(&PersistenceType::from(circuit.persistence))
            .with_durability(&DurabilityType::from(circuit.durability))
            .with_routes(&RouteType::from(circuit.routes))
            .with_circuit_management_type(&circuit.circuit_management_type)
            .with_application_metadata(&parse_hex(&circuit.application_metadata).map_err(|_| {
                InvalidStateError::with_message("Requester public key is not valid hex".to_string())
            })?)
            .with_comments(&circuit.comments)
            .build()
    }
}

impl From<ProposedCircuit> for YamlProposedCircuit {
    fn from(circuit: ProposedCircuit) -> Self {
        YamlProposedCircuit {
            circuit_id: circuit.circuit_id().into(),
            roster: circuit
                .roster()
                .to_vec()
                .into_iter()
                .map(YamlProposedService::from)
                .collect(),
            members: circuit
                .members()
                .to_vec()
                .into_iter()
                .map(YamlProposedNode::from)
                .collect(),
            authorization_type: circuit.authorization_type().clone().into(),
            persistence: circuit.persistence().clone().into(),
            durability: circuit.durability().clone().into(),
            routes: circuit.routes().clone().into(),
            circuit_management_type: circuit.circuit_management_type().into(),
            application_metadata: to_hex(circuit.application_metadata()),
            comments: circuit.comments().into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default, Eq)]
pub struct YamlProposedService {
    service_id: String,
    service_type: String,
    allowed_nodes: Vec<String>,
    arguments: Vec<(String, String)>,
}

impl TryFrom<YamlProposedService> for ProposedService {
    type Error = InvalidStateError;

    fn try_from(service: YamlProposedService) -> Result<Self, Self::Error> {
        ProposedServiceBuilder::new()
            .with_service_id(&service.service_id)
            .with_service_type(&service.service_type)
            .with_node_id(&service.allowed_nodes.get(0).ok_or_else(|| {
                InvalidStateError::with_message("Must contain 1 node ID".to_string())
            })?)
            .with_arguments(
                &service
                    .arguments
                    .iter()
                    .map(|(key, value)| (key.to_string(), value.to_string()))
                    .collect::<Vec<(String, String)>>(),
            )
            .build()
    }
}

impl From<ProposedService> for YamlProposedService {
    fn from(service: ProposedService) -> Self {
        YamlProposedService {
            service_id: service.service_id().into(),
            service_type: service.service_type().into(),
            allowed_nodes: vec![service.node_id().to_string()],
            arguments: service
                .arguments()
                .iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        }
    }
}

/// YAML file specific ProposedNode definition.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct YamlProposedNode {
    node_id: String,
    endpoints: Vec<String>,
}

impl TryFrom<YamlProposedNode> for ProposedNode {
    type Error = InvalidStateError;

    fn try_from(node: YamlProposedNode) -> Result<Self, Self::Error> {
        ProposedNodeBuilder::new()
            .with_node_id(&node.node_id)
            .with_endpoints(&node.endpoints)
            .build()
    }
}

impl From<ProposedNode> for YamlProposedNode {
    fn from(node: ProposedNode) -> Self {
        YamlProposedNode {
            node_id: node.node_id().into(),
            endpoints: node.endpoints().into(),
        }
    }
}

/// YAML file specific AuthorizationType definition for serialization.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum YamlAuthorizationType {
    Trust,
}

impl From<AuthorizationType> for YamlAuthorizationType {
    fn from(authorization_type: AuthorizationType) -> Self {
        match authorization_type {
            AuthorizationType::Trust => YamlAuthorizationType::Trust,
        }
    }
}

impl From<YamlAuthorizationType> for AuthorizationType {
    fn from(yaml_authorization_type: YamlAuthorizationType) -> Self {
        match yaml_authorization_type {
            YamlAuthorizationType::Trust => AuthorizationType::Trust,
        }
    }
}

/// YAML file specific PersistenceType definition for serialization.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum YamlPersistenceType {
    Any,
}

impl From<PersistenceType> for YamlPersistenceType {
    fn from(persistence_type: PersistenceType) -> Self {
        match persistence_type {
            PersistenceType::Any => YamlPersistenceType::Any,
        }
    }
}

impl From<YamlPersistenceType> for PersistenceType {
    fn from(yaml_persistence_type: YamlPersistenceType) -> Self {
        match yaml_persistence_type {
            YamlPersistenceType::Any => PersistenceType::Any,
        }
    }
}

/// YAML file specific DurabilityType definition for serialization.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum YamlDurabilityType {
    NoDurability,
}

impl From<DurabilityType> for YamlDurabilityType {
    fn from(durability_type: DurabilityType) -> Self {
        match durability_type {
            DurabilityType::NoDurability => YamlDurabilityType::NoDurability,
        }
    }
}

impl From<YamlDurabilityType> for DurabilityType {
    fn from(yaml_durability_type: YamlDurabilityType) -> Self {
        match yaml_durability_type {
            YamlDurabilityType::NoDurability => DurabilityType::NoDurability,
        }
    }
}

/// YAML file specific RouteType definition for serialization.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum YamlRouteType {
    Any,
}

impl From<RouteType> for YamlRouteType {
    fn from(route_type: RouteType) -> Self {
        match route_type {
            RouteType::Any => YamlRouteType::Any,
        }
    }
}

impl From<YamlRouteType> for RouteType {
    fn from(yaml_route_type: YamlRouteType) -> Self {
        match yaml_route_type {
            YamlRouteType::Any => RouteType::Any,
        }
    }
}

/// YAML file specific CircuitNode definition for serialization.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct YamlCircuitNode {
    id: String,
    endpoints: Vec<String>,
}

impl From<CircuitNode> for YamlCircuitNode {
    fn from(circuit_node: CircuitNode) -> Self {
        YamlCircuitNode {
            id: circuit_node.node_id().to_string(),
            endpoints: circuit_node.endpoints().to_vec(),
        }
    }
}

impl TryFrom<YamlCircuitNode> for CircuitNode {
    type Error = InvalidStateError;

    fn try_from(yaml_circuit_node: YamlCircuitNode) -> Result<Self, Self::Error> {
        CircuitNodeBuilder::new()
            .with_node_id(&yaml_circuit_node.id)
            .with_endpoints(&yaml_circuit_node.endpoints)
            .build()
    }
}

/// YAML file specific Vote definition for serialization.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum YamlVote {
    Accept,
    Reject,
}

impl From<Vote> for YamlVote {
    fn from(vote: Vote) -> Self {
        match vote {
            Vote::Accept => YamlVote::Accept,
            Vote::Reject => YamlVote::Reject,
        }
    }
}

impl From<YamlVote> for Vote {
    fn from(vote: YamlVote) -> Self {
        match vote {
            YamlVote::Accept => Vote::Accept,
            YamlVote::Reject => Vote::Reject,
        }
    }
}

/// YAML file specific state definition that can be read and written to the proposal YAML state file
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
struct YamlProposalState {
    proposals: BTreeMap<String, YamlCircuitProposal>,
}

/// The proposal state that is cached by the YAML admin service store and used to respond to fetch
/// requests
#[derive(Debug, Clone, PartialEq, Default)]
struct ProposalState {
    proposals: BTreeMap<String, CircuitProposal>,
}

/// The combination of circuit and circuit proposal state
#[derive(Debug, Clone, Default)]
struct YamlState {
    circuit_state: CircuitState,
    proposal_state: ProposalState,
    service_directory: BTreeMap<ServiceId, Service>,
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use tempdir::TempDir;

    use super::*;

    use crate::admin::store::{
        CircuitNodeBuilder, CircuitProposalBuilder, ProposalType, ProposedCircuitBuilder,
        ProposedNodeBuilder, ProposedServiceBuilder, Vote, VoteRecordBuilder,
    };
    use crate::hex::parse_hex;

    const CIRCUIT_STATE: &[u8] = b"---
nodes:
    acme-node-000:
        id: acme-node-000
        endpoints:
          - \"tcps://splinterd-node-acme:8044\"
    bubba-node-000:
        id: bubba-node-000
        endpoints:
          - \"tcps://splinterd-node-bubba:8044\"
circuits:
    WBKLF-AAAAA:
        id: WBKLF-AAAAA
        auth: Trust
        members:
          - bubba-node-000
          - acme-node-000
        roster:
          - service_id: a000
            service_type: scabbard
            allowed_nodes:
              - acme-node-000
            arguments:
              admin_keys: '[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]'
              peer_services: '[\"a001\"]'
          - service_id: a001
            service_type: scabbard
            allowed_nodes:
              - bubba-node-000
            arguments:
              admin_keys: '[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]'
              peer_services: '[\"a000\"]'
        persistence: Any
        durability: NoDurability
        routes: Any
        circuit_management_type: gameroom";

    const PROPOSAL_STATE: &[u8] = b"---
proposals:
    WBKLF-BBBBB:
        proposal_type: Create
        circuit_id: WBKLF-BBBBB
        circuit_hash: 7ddc426972710adc0b2ecd49e89a9dd805fb9206bf516079724c887bedbcdf1d
        circuit:
            circuit_id: WBKLF-BBBBB
            roster:
            - service_id: a000
              service_type: scabbard
              allowed_nodes:
                - acme-node-000
              arguments:
                - - peer_services
                  - '[\"a001\"]'
                - - admin_keys
                  - '[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]'
            - service_id: a001
              service_type: scabbard
              allowed_nodes:
                - bubba-node-000
              arguments:
                - - peer_services
                  - '[\"a000\"]'
                - - admin_keys
                  - '[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]'
            members:
            - node_id: bubba-node-000
              endpoints:
                - \"tcps://splinterd-node-bubba:8044\"
            - node_id: acme-node-000
              endpoints:
                - \"tcps://splinterd-node-acme:8044\"
            authorization_type: Trust
            persistence: Any
            durability: NoDurability
            routes: Any
            circuit_management_type: gameroom
            application_metadata: ''
            comments: \"\"
        votes: []
        requester: 0283a14e0a17cb7f665311e9b5560f4cde2b502f17e2d03223e15d90d9318d7482
        requester_node_id: acme-node-000";

    // Validate that if the YAML state files do not exist, the YamlAdminServiceStore will create
    // the files with empty states.
    //
    // 1. Creates a empty temp directory
    // 2. Create a YAML admin service directory
    // 3. Validate that the circuit and proposals YAMLfiles were created in the temp dir.
    #[test]
    fn test_write_new_files() {
        let temp_dir = TempDir::new("test_write_new_files").expect("Failed to create temp dir");
        let circuit_path = temp_dir
            .path()
            .join("circuits.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        let proposals_path = temp_dir
            .path()
            .join("circuit_proposals.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        // validate the files do not exist
        assert!(!PathBuf::from(circuit_path.clone()).is_file());
        assert!(!PathBuf::from(proposals_path.clone()).is_file());

        // create YamlAdminServiceStore
        let _store = YamlAdminServiceStore::new(circuit_path.clone(), proposals_path.clone())
            .expect("Unable to create yaml admin store");

        // validate the files exist now
        assert!(PathBuf::from(circuit_path.clone()).is_file());
        assert!(PathBuf::from(proposals_path.clone()).is_file());
    }

    // Validate that the YAML admin service store can properly load circuit and proposals state
    // from existing YAML files
    //
    // 1. Creates a temp directory with existing circuit and proposals yaml files
    // 2. Create a YAML admin service directory
    // 3. Validate that the circuit and proposals can be fetched from state
    #[test]
    fn test_read_existing_files() {
        // create temp dir
        let temp_dir = TempDir::new("test_read_existing_files").expect("Failed to create temp dir");
        let circuit_path = temp_dir
            .path()
            .join("circuits.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        let proposals_path = temp_dir
            .path()
            .join("circuit_proposals.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        // write yaml files to temp_dir
        write_file(CIRCUIT_STATE, &circuit_path);
        write_file(PROPOSAL_STATE, &proposals_path);

        // create YamlAdminServiceStore
        let store = YamlAdminServiceStore::new(circuit_path.clone(), proposals_path.clone())
            .expect("Unable to create yaml admin store");

        assert!(store
            .get_proposal("WBKLF-BBBBB")
            .expect("unable to fetch proposals")
            .is_some());
        assert!(store
            .get_circuit("WBKLF-AAAAA")
            .expect("unable to fetch circuits")
            .is_some());
    }

    // Test the proposal CRUD operations
    //
    // 1. Setup the temp directory with existing state
    // 2. Fetch an existing proposal from state, validate proposal is returned
    // 3. Fetch an non exisitng proposal from state, validate None
    // 4. Update fetched proposal with a vote record and update, validate ok
    // 5. Call update with new proposal, validate error is returned
    // 6. Add new proposal, validate ok
    // 7. List proposal, validate both the updated original proposal and new proposal is returned
    // 8. Remove original proposal, validate okay
    // 9. Validate the proposal state YAML in the temp dir matches the expected bytes and only
    //    the new proposals
    #[test]
    fn test_proposals() {
        // create temp dir
        let temp_dir = TempDir::new("test_proposals").expect("Failed to create temp dir");
        let circuit_path = temp_dir
            .path()
            .join("circuits.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        let proposals_path = temp_dir
            .path()
            .join("circuit_proposals.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        // write yaml files to temp_dir
        write_file(CIRCUIT_STATE, &circuit_path);
        write_file(PROPOSAL_STATE, &proposals_path);

        // create YamlAdminServiceStore
        let store = YamlAdminServiceStore::new(circuit_path.clone(), proposals_path.clone())
            .expect("Unable to create yaml admin store");

        // fetch existing proposal from state
        let proposal = store
            .get_proposal("WBKLF-BBBBB")
            .expect("unable to fetch proposals")
            .expect("Expected proposal, got none");

        assert_eq!(proposal, create_expected_proposal());

        // fetch nonexisting proposal from state
        assert!(store
            .get_proposal("WBKLF-BADD")
            .expect("unable to fetch proposals")
            .is_none());

        let updated_proposal = proposal
            .builder()
            .with_votes(&vec![VoteRecordBuilder::new()
                .with_public_key(
                    &parse_hex(
                        "035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550",
                    )
                    .unwrap(),
                )
                .with_vote(&Vote::Accept)
                .with_voter_node_id("bubba-node-000")
                .build()
                .expect("Unable to build vote record")])
            .build()
            .expect("Unable to build updated proposal");

        store
            .update_proposal(updated_proposal.clone())
            .expect("Unable to update proposal");

        let new_proposal = new_proposal();

        assert!(
            store.update_proposal(new_proposal.clone()).is_err(),
            "Updating new proposal should fail"
        );

        store
            .add_proposal(new_proposal.clone())
            .expect("Unable to add proposal");

        assert_eq!(
            store
                .list_proposals(&vec![])
                .expect("Unable to get list of proposals")
                .collect::<Vec<CircuitProposal>>(),
            vec![updated_proposal, new_proposal.clone()]
        );

        store
            .remove_proposal("WBKLF-BBBBB")
            .expect("Unable to remove proposals");

        let mut yaml_state = BTreeMap::new();
        yaml_state.insert(
            new_proposal.circuit_id().to_string(),
            YamlCircuitProposal::from(new_proposal),
        );

        let mut yaml_state_vec = serde_yaml::to_vec(&YamlProposalState {
            proposals: yaml_state,
        })
        .unwrap();

        // Add new line because the file has a new added to it
        yaml_state_vec.append(&mut "\n".as_bytes().to_vec());

        let mut contents = vec![];
        File::open(proposals_path.clone())
            .unwrap()
            .read_to_end(&mut contents)
            .expect("Unable to read proposals");

        assert_eq!(yaml_state_vec, contents)
    }

    // Test the circuit CRUD operations
    //
    // 1. Setup the temp directory with existing state
    // 2. Fetch an existing circuit from state, validate circuit is returned
    // 3. Fetch an non exisitng circuit from state, validate None
    // 4. Update fetched proposa with a vote record and update, validate ok
    // 5. Call update with new circuit, validate error is returned
    // 6. Add new circuit, validate ok
    // 7. List circuit, validate both the updated original circuit and new circuit is returned
    // 8. Remove original circuit, validate okay
    // 9. Validate the circuit state YAML in the temp dir matches the expected bytes and contains
    //    only the new circuit
    #[test]
    fn test_circuit() {
        // create temp dir
        let temp_dir = TempDir::new("test_circuit").expect("Failed to create temp dir");
        let circuit_path = temp_dir
            .path()
            .join("circuits.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        let proposals_path = temp_dir
            .path()
            .join("circuit_proposals.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        // write yaml files to temp_dir
        write_file(CIRCUIT_STATE, &circuit_path);
        write_file(PROPOSAL_STATE, &proposals_path);

        // create YamlAdminServiceStore
        let store = YamlAdminServiceStore::new(circuit_path.clone(), proposals_path.clone())
            .expect("Unable to create yaml admin store");

        // fetch existing circuit from state
        let circuit = store
            .get_circuit("WBKLF-AAAAA")
            .expect("unable to fetch circuit")
            .expect("Expected circuit, got none");

        assert_eq!(circuit, create_expected_circuit());

        // fetch nonexisting circuitfrom state
        assert!(store
            .get_circuit("WBKLF-BADD")
            .expect("unable to fetch circuit")
            .is_none());

        let updated_circuit = CircuitBuilder::default()
                .with_circuit_id("WBKLF-AAAAA")
                .with_roster(&vec![
                    ServiceBuilder::default()
                        .with_service_id("a000")
                        .with_service_type("scabbard")
                        .with_node_id("acme-node-000")
                        .with_arguments(&vec![
                            ("admin_keys".into(),
                           "[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]"
                                .into()),
                           ("peer_services".into(), "[\"a001\"]".into()),
                        ])
                        .build()
                        .expect("Unable to build service"),
                    ServiceBuilder::default()
                        .with_service_id("a001")
                        .with_service_type("scabbard")
                        .with_node_id("bubba-node-000")
                        .with_arguments(&vec![(
                            "admin_keys".into(),
                            "[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]"
                                .into()
                        ),(
                            "peer_services".into(), "[\"a000\"]".into()
                        )])
                        .build()
                        .expect("Unable to build service"),
                ])
                .with_members(&vec!["bubba-node-000".into(), "acme-node-000".into()])
                .with_circuit_management_type("test")
                .build()
                .expect("Unable to build circuit");

        store
            .update_circuit(updated_circuit.clone())
            .expect("Unable to update circuit");

        let (new_circuit, new_node) = new_circuit();

        assert!(
            store.update_circuit(new_circuit.clone()).is_err(),
            "Updating new cirucit should fail"
        );

        store
            .add_circuit(new_circuit.clone(), vec![new_node.clone()])
            .expect("Unable to add cirucit");

        assert_eq!(
            store
                .list_circuits(&vec![])
                .expect("Unable to get list of circuits")
                .collect::<Vec<Circuit>>(),
            vec![updated_circuit, new_circuit.clone()]
        );

        store
            .remove_circuit("WBKLF-AAAAA")
            .expect("Unable to remove circuit");

        let mut yaml_circuits = BTreeMap::new();
        let mut yaml_nodes = BTreeMap::new();
        yaml_circuits.insert(
            new_circuit.circuit_id().to_string(),
            YamlCircuit::from(new_circuit),
        );
        yaml_nodes.insert(
            "acme-node-000".to_string(),
            YamlCircuitNode::from(
                CircuitNodeBuilder::new()
                    .with_node_id("acme-node-000")
                    .with_endpoints(&["tcps://splinterd-node-acme:8044".into()])
                    .build()
                    .expect("Unable to build circuit node"),
            ),
        );
        yaml_nodes.insert(
            "bubba-node-000".to_string(),
            YamlCircuitNode::from(
                CircuitNodeBuilder::new()
                    .with_node_id("bubba-node-000")
                    .with_endpoints(&["tcps://splinterd-node-bubba:8044".into()])
                    .build()
                    .expect("Unable to build circuit node"),
            ),
        );
        yaml_nodes.insert(
            new_node.node_id().to_string(),
            YamlCircuitNode::from(new_node),
        );
        let mut yaml_state_vec = serde_yaml::to_vec(&YamlCircuitState {
            circuits: yaml_circuits,
            nodes: yaml_nodes,
        })
        .unwrap();

        // Add new line because the file has a new added to it
        yaml_state_vec.append(&mut "\n".as_bytes().to_vec());

        let mut contents = vec![];
        File::open(circuit_path.clone())
            .unwrap()
            .read_to_end(&mut contents)
            .expect("Unable to read proposals");

        assert_eq!(yaml_state_vec, contents)
    }

    // Test the node CRUD operations
    //
    // 1. Setup the temp directory with existing state
    // 2. Check that the expected node is returned when fetched
    // 3. Check that the expected nodes are returned when list_nodes is called
    #[test]
    fn test_node() {
        // create temp dir
        let temp_dir = TempDir::new("test_node").expect("Failed to create temp dir");
        let circuit_path = temp_dir
            .path()
            .join("circuits.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        let proposals_path = temp_dir
            .path()
            .join("circuit_proposals.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        // write yaml files to temp_dir
        write_file(CIRCUIT_STATE, &circuit_path);
        write_file(PROPOSAL_STATE, &proposals_path);

        // create YamlAdminServiceStore
        let store = YamlAdminServiceStore::new(circuit_path.clone(), proposals_path.clone())
            .expect("Unable to create yaml admin store");

        let node = store
            .get_node("acme-node-000")
            .expect("Unable to fetch node")
            .expect("expected node, got none");

        assert_eq!(
            node,
            CircuitNodeBuilder::new()
                .with_node_id("acme-node-000")
                .with_endpoints(&["tcps://splinterd-node-acme:8044".into()])
                .build()
                .expect("Unable to build circuit node"),
        );

        assert_eq!(
            store.list_nodes().unwrap().collect::<Vec<CircuitNode>>(),
            vec![
                CircuitNodeBuilder::new()
                    .with_node_id("acme-node-000")
                    .with_endpoints(&["tcps://splinterd-node-acme:8044".into()])
                    .build()
                    .expect("Unable to build circuit node"),
                CircuitNodeBuilder::new()
                    .with_node_id("bubba-node-000")
                    .with_endpoints(&["tcps://splinterd-node-bubba:8044".into()])
                    .build()
                    .expect("Unable to build circuit node"),
            ]
        );
    }

    // Test the service CRUD operations
    //
    // 1. Setup the temp directory with existing state
    // 2. Check that the expected service is returned when fetched
    // 3. Check that the expected services are returned when list_services is called
    #[test]
    fn test_service() {
        // create temp dir
        let temp_dir = TempDir::new("test_service").expect("Failed to create temp dir");
        let circuit_path = temp_dir
            .path()
            .join("circuits.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        let proposals_path = temp_dir
            .path()
            .join("circuit_proposals.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        // write yaml files to temp_dir
        write_file(CIRCUIT_STATE, &circuit_path);
        write_file(PROPOSAL_STATE, &proposals_path);

        let service_id = ServiceId::new("a000".to_string(), "WBKLF-AAAAA".to_string());

        // create YamlAdminServiceStore
        let store = YamlAdminServiceStore::new(circuit_path.clone(), proposals_path.clone())
            .expect("Unable to create yaml admin store");

        let service = store
            .get_service(&service_id)
            .expect("Unable to fetch service")
            .expect("unable to get expected service, got none");

        assert_eq!(
            service,
            ServiceBuilder::default()
                .with_service_id("a000")
                .with_service_type("scabbard")
                .with_node_id("acme-node-000")
                .with_arguments(&vec![
                    (
                        "admin_keys".into(),
                        "[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]"
                            .into()
                    ),
                    ("peer_services".into(), "[\"a001\"]".into()),
                ])
                .build()
                .expect("Unable to build service"),
        );

        assert_eq!(
            store
                .list_services("WBKLF-AAAAA")
                .unwrap()
                .collect::<Vec<Service>>(),
            vec![
                ServiceBuilder::default()
                    .with_service_id("a000")
                    .with_service_type("scabbard")
                    .with_node_id("acme-node-000")
                    .with_arguments(&vec![
                    ("admin_keys".into(),
                   "[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]"
                   .into()),
                   ("peer_services".into(), "[\"a001\"]".into()),
                ])
                    .build()
                    .expect("Unable to build service"),
                ServiceBuilder::default()
                    .with_service_id("a001")
                    .with_service_type("scabbard")
                    .with_node_id("bubba-node-000")
                    .with_arguments(&vec![
                        ("admin_keys".into(),
                       "[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]"
                       .into()),
                           ("peer_services".into(), "[\"a000\"]".into()),
                    ])
                    .build()
                    .expect("Unable to build service")
            ]
        );
    }

    // Test that a proposals can be upgraded to a circuit and both yaml files are upgraded.
    //
    // 1. Setup the temp directory with existing proposal state
    // 2. Upgrade proposal to circuit, validate ok
    // 3. Check that proposals are now empty
    // 4. Check that the circuit, nodes and services have been set
    #[test]
    fn test_upgrading_proposals_to_circuit() {
        // create temp dir
        let temp_dir =
            TempDir::new("est_upgrading_proposals_to_circuit").expect("Failed to create temp dir");
        let circuit_path = temp_dir
            .path()
            .join("circuits.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        let proposals_path = temp_dir
            .path()
            .join("circuit_proposals.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        // write proposal to state
        write_file(PROPOSAL_STATE, &proposals_path);

        // create YamlAdminServiceStore
        let store = YamlAdminServiceStore::new(circuit_path.clone(), proposals_path.clone())
            .expect("Unable to create yaml admin store");

        let service_id = ServiceId::new("a000".to_string(), "WBKLF-BBBBB".to_string());
        assert_eq!(store.get_circuit("WBKLF-BBBBB").unwrap(), None);
        assert_eq!(store.get_node("acme-node-000").unwrap(), None);
        assert_eq!(store.get_service(&service_id).unwrap(), None);

        store
            .upgrade_proposal_to_circuit("WBKLF-BBBBB")
            .expect("Unable to upgrade proposalto circuit");

        assert_eq!(store.list_proposals(&vec![]).unwrap().next(), None);

        assert!(store.get_circuit("WBKLF-BBBBB").unwrap().is_some());
        assert!(store.get_node("acme-node-000").unwrap().is_some());
        assert!(store.get_service(&service_id).unwrap().is_some());
    }

    fn write_file(data: &[u8], file_path: &str) {
        let mut file = File::create(file_path).expect("Error creating test yaml file.");
        file.write_all(data)
            .expect("unable to write test file to temp dir")
    }

    fn create_expected_proposal() -> CircuitProposal {
        CircuitProposalBuilder::default()
            .with_proposal_type(&ProposalType::Create)
            .with_circuit_id("WBKLF-BBBBB")
            .with_circuit_hash(
                "7ddc426972710adc0b2ecd49e89a9dd805fb9206bf516079724c887bedbcdf1d")
            .with_circuit(
                &ProposedCircuitBuilder::default()
                    .with_circuit_id("WBKLF-BBBBB")
                    .with_roster(&vec![
                        ProposedServiceBuilder::default()
                            .with_service_id("a000")
                            .with_service_type("scabbard")
                            .with_node_id(&"acme-node-000")
                            .with_arguments(&vec![
                                ("peer_services".into(), "[\"a001\"]".into()),
                                ("admin_keys".into(),
                               "[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]".into())
                            ])
                            .build().expect("Unable to build service"),
                        ProposedServiceBuilder::default()
                            .with_service_id("a001")
                            .with_service_type("scabbard")
                            .with_node_id(&"bubba-node-000")
                            .with_arguments(&vec![
                                ("peer_services".into(), "[\"a000\"]".into()),
                                ("admin_keys".into(),
                               "[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]".into())
                            ])
                            .build().expect("Unable to build service")
                        ])

                    .with_members(
                        &vec![
                        ProposedNodeBuilder::default()
                            .with_node_id("bubba-node-000".into())
                            .with_endpoints(&vec!["tcps://splinterd-node-bubba:8044".into()])
                            .build().expect("Unable to build node"),
                        ProposedNodeBuilder::default()
                            .with_node_id("acme-node-000".into())
                            .with_endpoints(&vec!["tcps://splinterd-node-acme:8044".into()])
                            .build().expect("Unable to build node"),
                        ]
                    )
                    .with_circuit_management_type("gameroom")
                    .build().expect("Unable to build circuit")
            )
            .with_requester(
                &parse_hex(
                    "0283a14e0a17cb7f665311e9b5560f4cde2b502f17e2d03223e15d90d9318d7482").unwrap())
            .with_requester_node_id("acme-node-000")
            .build().expect("Unable to build proposals")
    }

    fn create_expected_circuit() -> Circuit {
        CircuitBuilder::default()
            .with_circuit_id("WBKLF-AAAAA")
            .with_roster(&vec![
                ServiceBuilder::default()
                    .with_service_id("a000")
                    .with_service_type("scabbard")
                    .with_node_id("acme-node-000")
                    .with_arguments(&vec![
                        ("admin_keys".into(),
                       "[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]"
                            .into()),
                       ("peer_services".into(), "[\"a001\"]".into()),
                    ])
                    .build()
                    .expect("Unable to build service"),
                ServiceBuilder::default()
                    .with_service_id("a001")
                    .with_service_type("scabbard")
                    .with_node_id("bubba-node-000")
                    .with_arguments(&vec![(
                        "admin_keys".into(),
                        "[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]"
                            .into()
                    ),(
                        "peer_services".into(), "[\"a000\"]".into()
                    )])
                    .build()
                    .expect("Unable to build service"),
            ])
            .with_members(&vec!["bubba-node-000".into(), "acme-node-000".into()])
            .with_circuit_management_type("gameroom")
            .build()
            .expect("Unable to build circuit")
    }

    fn new_proposal() -> CircuitProposal {
        CircuitProposalBuilder::default()
            .with_proposal_type(&ProposalType::Create)
            .with_circuit_id("WBKLF-CCCCC")
            .with_circuit_hash(
                "7ddc426972710adc0b2ecd49e89a9dd805fb9206bf516079724c887bedbcdf1d")
            .with_circuit(
                &ProposedCircuitBuilder::default()
                    .with_circuit_id("WBKLF-PqfoE")
                    .with_roster(&vec![
                        ProposedServiceBuilder::default()
                            .with_service_id("a000")
                            .with_service_type("scabbard")
                            .with_node_id("acme-node-000")
                            .with_arguments(&vec![
                                ("peer_services".into(), "[\"a001\"]".into()),
                                ("admin_keys".into(),
                               "[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]".into())
                            ])
                            .build().expect("Unable to build service"),
                        ProposedServiceBuilder::default()
                            .with_service_id("a001")
                            .with_service_type("scabbard")
                            .with_node_id("bubba-node-000")
                            .with_arguments(&vec![
                                ("peer_services".into(), "[\"a000\"]".into()),
                                ("admin_keys".into(),
                               "[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]".into())
                            ])
                            .build().expect("Unable to build service")
                        ])

                    .with_members(
                        &vec![
                        ProposedNodeBuilder::default()
                            .with_node_id("bubba-node-000".into())
                            .with_endpoints(&vec!["tcps://splinterd-node-bubba:8044".into()])
                            .build().expect("Unable to build node"),
                        ProposedNodeBuilder::default()
                            .with_node_id("acme-node-000".into())
                            .with_endpoints(&vec!["tcps://splinterd-node-acme:8044".into()])
                            .build().expect("Unable to build node"),
                        ]
                    )
                    .with_circuit_management_type("test")
                    .build().expect("Unable to build circuit")
            )
            .with_requester(
                &parse_hex(
                    "0283a14e0a17cb7f665311e9b5560f4cde2b502f17e2d03223e15d90d9318d7482").unwrap())
            .with_requester_node_id("acme-node-000")
            .build().expect("Unable to build proposals")
    }

    fn new_circuit() -> (Circuit, CircuitNode) {
        (CircuitBuilder::default()
            .with_circuit_id("WBKLF-DDDDD")
            .with_roster(&vec![
                ServiceBuilder::default()
                    .with_service_id("a000")
                    .with_service_type("scabbard")
                    .with_node_id("acme-node-000")
                    .with_arguments(&vec![
                        ("peer_services".into(), "[\"a001\"]".into()),
                        ("admin_keys".into(),
                       "[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]".into())
                    ])
                    .build().expect("Unable to build service"),
                ServiceBuilder::default()
                    .with_service_id("a001")
                    .with_service_type("scabbard")
                    .with_node_id("bubba-node-000")
                    .with_arguments(&vec![
                        ("peer_services".into(), "[\"a000\"]".into()),
                        ("admin_keys".into(),
                       "[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]".into())
                    ])
                    .build().expect("Unable to build service")
                ])
            .with_members(
                &vec![
                    "bubba-node-000".into(),
                    "acme-node-000".into(),
                    "new-node-000".into()
                ]
            )
            .with_circuit_management_type("test")
            .build().expect("Unable to build circuit"),
        CircuitNodeBuilder::default()
            .with_node_id("new-node-000".into())
            .with_endpoints(&vec!["tcps://splinterd-node-new:8044".into()])
            .build().expect("Unable to build node"))
    }
}
