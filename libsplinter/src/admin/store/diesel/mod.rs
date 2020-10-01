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

//! Database backend support for the AdminServiceStore, powered by
//! [`Diesel`](https://crates.io/crates/diesel).
//!
//! This module contains the [`DieselAdminServiceStore`], which provides an implementation of the
//! [`AdminServiceStore`] trait.
//!
//! [`DieselAdminServiceStore`]: struct.DieselAdminServiceStore.html
//! [`AdminServiceStore`]: ../trait.AdminServiceStore.html

pub mod migrations;
mod models;
mod operations;
mod schema;

use diesel::r2d2::{ConnectionManager, Pool};

use crate::admin::store::{
    error::AdminServiceStoreError, AdminServiceStore, Circuit, CircuitNode, CircuitPredicate,
    CircuitProposal, Service, ServiceId,
};
use operations::add_circuit::AdminServiceStoreAddCircuitOperation as _;
use operations::add_proposal::AdminServiceStoreAddProposalOperation as _;
use operations::get_circuit::AdminServiceStoreFetchCircuitOperation as _;
use operations::get_node::AdminServiceStoreFetchNodeOperation as _;
use operations::get_proposal::AdminServiceStoreFetchProposalOperation as _;
use operations::get_service::AdminServiceStoreFetchServiceOperation as _;
use operations::list_circuits::AdminServiceStoreListCircuitsOperation as _;
use operations::list_nodes::AdminServiceStoreListNodesOperation as _;
use operations::list_proposals::AdminServiceStoreListProposalsOperation as _;
use operations::list_services::AdminServiceStoreListServicesOperation as _;
use operations::remove_circuit::AdminServiceStoreRemoveCircuitOperation as _;
use operations::remove_proposal::AdminServiceStoreRemoveProposalOperation as _;
use operations::update_circuit::AdminServiceStoreUpdateCircuitOperation as _;
use operations::update_proposal::AdminServiceStoreUpdateProposalOperation as _;
use operations::upgrade::AdminServiceStoreUpgradeProposalToCircuitOperation as _;
use operations::AdminServiceStoreOperations;

/// A database-backed AdminServiceStore, powered by [`Diesel`](https://crates.io/crates/diesel).
pub struct DieselAdminServiceStore<C: diesel::Connection + 'static> {
    connection_pool: Pool<ConnectionManager<C>>,
}

impl<C: diesel::Connection> DieselAdminServiceStore<C> {
    /// Creates a new `DieselAdminServiceStore`.
    ///
    /// # Arguments
    ///
    ///  * `connection_pool`: connection pool for the database
    pub fn new(connection_pool: Pool<ConnectionManager<C>>) -> Self {
        DieselAdminServiceStore { connection_pool }
    }
}

#[cfg(feature = "sqlite")]
impl Clone for DieselAdminServiceStore<diesel::sqlite::SqliteConnection> {
    fn clone(&self) -> Self {
        Self {
            connection_pool: self.connection_pool.clone(),
        }
    }
}

#[cfg(feature = "postgres")]
impl Clone for DieselAdminServiceStore<diesel::pg::PgConnection> {
    fn clone(&self) -> Self {
        Self {
            connection_pool: self.connection_pool.clone(),
        }
    }
}

#[cfg(feature = "postgres")]
impl AdminServiceStore for DieselAdminServiceStore<diesel::pg::PgConnection> {
    fn add_proposal(&self, proposal: CircuitProposal) -> Result<(), AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).add_proposal(proposal)
    }

    fn update_proposal(&self, proposal: CircuitProposal) -> Result<(), AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).update_proposal(proposal)
    }

    fn remove_proposal(&self, proposal_id: &str) -> Result<(), AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).remove_proposal(proposal_id)
    }

    fn get_proposal(
        &self,
        proposal_id: &str,
    ) -> Result<Option<CircuitProposal>, AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).get_proposal(proposal_id)
    }

    fn list_proposals(
        &self,
        predicates: &[CircuitPredicate],
    ) -> Result<Box<dyn ExactSizeIterator<Item = CircuitProposal>>, AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).list_proposals(predicates)
    }

    fn add_circuit(
        &self,
        circuit: Circuit,
        nodes: Vec<CircuitNode>,
    ) -> Result<(), AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).add_circuit(circuit, nodes)
    }

    fn update_circuit(&self, circuit: Circuit) -> Result<(), AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).update_circuit(circuit)
    }

    fn remove_circuit(&self, circuit_id: &str) -> Result<(), AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).remove_circuit(circuit_id)
    }

    fn get_circuit(&self, circuit_id: &str) -> Result<Option<Circuit>, AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).get_circuit(circuit_id)
    }

    fn list_circuits(
        &self,
        predicates: &[CircuitPredicate],
    ) -> Result<Box<dyn ExactSizeIterator<Item = Circuit>>, AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).list_circuits(predicates)
    }

    fn upgrade_proposal_to_circuit(&self, circuit_id: &str) -> Result<(), AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?)
            .upgrade_proposal_to_circuit(circuit_id)
    }

    fn get_node(&self, node_id: &str) -> Result<Option<CircuitNode>, AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).get_node(node_id)
    }

    fn list_nodes(
        &self,
    ) -> Result<Box<dyn ExactSizeIterator<Item = CircuitNode>>, AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).list_nodes()
    }

    fn get_service(
        &self,
        service_id: &ServiceId,
    ) -> Result<Option<Service>, AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).get_service(service_id)
    }

    fn list_services(
        &self,
        circuit_id: &str,
    ) -> Result<Box<dyn ExactSizeIterator<Item = Service>>, AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).list_services(circuit_id)
    }
}

#[cfg(feature = "sqlite")]
impl AdminServiceStore for DieselAdminServiceStore<diesel::sqlite::SqliteConnection> {
    fn add_proposal(&self, proposal: CircuitProposal) -> Result<(), AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).add_proposal(proposal)
    }

    fn update_proposal(&self, proposal: CircuitProposal) -> Result<(), AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).update_proposal(proposal)
    }

    fn remove_proposal(&self, proposal_id: &str) -> Result<(), AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).remove_proposal(proposal_id)
    }

    fn get_proposal(
        &self,
        proposal_id: &str,
    ) -> Result<Option<CircuitProposal>, AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).get_proposal(proposal_id)
    }

    fn list_proposals(
        &self,
        predicates: &[CircuitPredicate],
    ) -> Result<Box<dyn ExactSizeIterator<Item = CircuitProposal>>, AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).list_proposals(predicates)
    }

    fn add_circuit(
        &self,
        circuit: Circuit,
        nodes: Vec<CircuitNode>,
    ) -> Result<(), AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).add_circuit(circuit, nodes)
    }

    fn update_circuit(&self, circuit: Circuit) -> Result<(), AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).update_circuit(circuit)
    }

    fn remove_circuit(&self, circuit_id: &str) -> Result<(), AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).remove_circuit(circuit_id)
    }

    fn get_circuit(&self, circuit_id: &str) -> Result<Option<Circuit>, AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).get_circuit(circuit_id)
    }

    fn list_circuits(
        &self,
        predicates: &[CircuitPredicate],
    ) -> Result<Box<dyn ExactSizeIterator<Item = Circuit>>, AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).list_circuits(predicates)
    }

    fn upgrade_proposal_to_circuit(&self, circuit_id: &str) -> Result<(), AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?)
            .upgrade_proposal_to_circuit(circuit_id)
    }

    fn get_node(&self, node_id: &str) -> Result<Option<CircuitNode>, AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).get_node(node_id)
    }

    fn list_nodes(
        &self,
    ) -> Result<Box<dyn ExactSizeIterator<Item = CircuitNode>>, AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).list_nodes()
    }

    fn get_service(
        &self,
        service_id: &ServiceId,
    ) -> Result<Option<Service>, AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).get_service(service_id)
    }

    fn list_services(
        &self,
        circuit_id: &str,
    ) -> Result<Box<dyn ExactSizeIterator<Item = Service>>, AdminServiceStoreError> {
        AdminServiceStoreOperations::new(&*self.connection_pool.get()?).list_services(circuit_id)
    }
}
