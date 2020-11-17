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

#[cfg(feature = "admin-service-store-postgres")]
impl Clone for DieselAdminServiceStore<diesel::pg::PgConnection> {
    fn clone(&self) -> Self {
        Self {
            connection_pool: self.connection_pool.clone(),
        }
    }
}

#[cfg(feature = "admin-service-store-postgres")]
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

#[cfg(all(test, feature = "sqlite"))]
pub mod tests {
    use super::*;

    use crate::admin::store::diesel::migrations::run_sqlite_migrations;
    use crate::admin::store::{
        CircuitBuilder, CircuitNodeBuilder, CircuitProposal, CircuitProposalBuilder, ProposalType,
        ProposedCircuitBuilder, ProposedNodeBuilder, ProposedServiceBuilder, ServiceBuilder, Vote,
        VoteRecordBuilder,
    };
    use crate::hex::parse_hex;

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };

    #[test]
    /// Test that the AdminServiceStore sqlite migrations can be run successfully
    fn test_sqlite_migrations() {
        create_connection_pool_and_migrate();
    }

    /// Verify that a proposal can be added to the store correctly and then fetched from the store
    ///
    /// 1. Run sqlite migrations
    /// 2. Create DieselAdminServiceStore
    /// 3. Create a proposal
    /// 4. Add proposal to store
    /// 5. Fetch Proposal from store
    /// 6. Validate fetched proposal is the same as the proposal added
    #[test]
    fn test_add_get_proposals() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselAdminServiceStore::new(pool);

        let proposal = create_proposal();

        store
            .add_proposal(proposal.clone())
            .expect("Unable to add circuit proposal");

        let fetched_proposal = store
            .get_proposal("WBKLF-BBBBB")
            .expect("Unable to get proposal")
            .expect("Got None when expecting proposal");

        assert_eq!(proposal, fetched_proposal);
    }

    /// Verify that list_proposals works correctly
    ///
    /// 1. Run sqlite migrations
    /// 2. Create DieselAdminServiceStore
    /// 3. Create a proposal
    /// 4. Add proposal to store
    /// 5. List Proposal from store with no predicates, validate added proposal is returned
    /// 6. List Proposal from store with management type predicate, validate added proposal is
    ///    returned
    /// 7. List Proposal from store with member predicate, validate added proposal is
    ///    returned
    /// 8. List Proposal from store with mismatching management type predicate, validate no
    ///    proposals are returned
    #[test]
    fn test_list_proposals() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselAdminServiceStore::new(pool);

        let proposal = create_proposal();

        store
            .add_proposal(proposal.clone())
            .expect("Unable to add circuit proposal");

        // test no predicates
        let mut proposals = store
            .list_proposals(&vec![])
            .expect("Unable to list proposals");

        assert_eq!(proposals.next(), Some(proposal.clone()));
        assert_eq!(proposals.next(), None);

        // test management type predicate
        let mut proposals = store
            .list_proposals(&vec![CircuitPredicate::ManagementTypeEq(
                "gameroom".to_string(),
            )])
            .expect("Unable to list proposals with management type predicate");

        assert_eq!(proposals.next(), Some(proposal.clone()));
        assert_eq!(proposals.next(), None);

        // test management type predicate
        let mut proposals = store
            .list_proposals(&vec![CircuitPredicate::ManagementTypeEq(
                "arcade".to_string(),
            )])
            .expect("Unable to list proposals with management type predicate");

        assert_eq!(proposals.next(), None);

        let extra_proposal = create_extra_proposal();

        store
            .add_proposal(extra_proposal.clone())
            .expect("Unable to add circuit proposal");

        // test management type predicate
        let mut proposals = store
            .list_proposals(&vec![CircuitPredicate::MembersInclude(vec![
                "gumbo-node-000".to_string(),
            ])])
            .expect("Unable to list proposals with members include predicate");

        assert_eq!(proposals.next(), Some(extra_proposal));
        assert_eq!(proposals.next(), None);

        let proposals = store
            .list_proposals(&vec![])
            .expect("Unable to list proposals with members include predicate");

        assert_eq!(proposals.len(), 2);
    }

    /// Verify that a proposal can be removed from the store
    ///
    /// 1. Run sqlite migrations
    /// 2. Create DieselAdminServiceStore
    /// 3. Create a proposal
    /// 4. Add proposal to store
    /// 5. Fetch Proposal from store
    /// 6. Validate fetched proposal is the same as the proposal added
    /// 7. Remove proposal
    /// 8. Validate the proposal was removed
    #[test]
    fn test_remove_proposals() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselAdminServiceStore::new(pool);

        let proposal = create_proposal();

        store
            .add_proposal(proposal.clone())
            .expect("Unable to add circuit proposal");

        let fetched_proposal = store
            .get_proposal("WBKLF-BBBBB")
            .expect("Unable to get proposal")
            .expect("Got None when expecting proposal");

        assert_eq!(proposal, fetched_proposal);

        store
            .remove_proposal("WBKLF-BBBBB")
            .expect("Unable to add circuit proposal");

        let fetched_proposal = store
            .get_proposal("WBKLF-BBBBB")
            .expect("Unable to get proposal");

        assert_eq!(None, fetched_proposal);
    }

    /// Verify that a proposal can be added to the store correctly and then updated from the store
    ///
    /// 1. Run sqlite migrations
    /// 2. Create DieselAdminServiceStore
    /// 3. Create a proposal
    /// 4. Add proposal to store
    /// 5. Fetch Proposal from store
    /// 6. Validate fetched proposal is the same as the proposal added
    /// 7. Update proposal to have a new vote and call update
    /// 8. Fetch Proposal from store
    /// 9. Validate fetched proposal now matches the updated proposal
    #[test]
    fn test_update_proposals() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselAdminServiceStore::new(pool);

        let proposal = create_proposal();

        store
            .add_proposal(proposal.clone())
            .expect("Unable to add circuit proposal");

        let fetched_proposal = store
            .get_proposal("WBKLF-BBBBB")
            .expect("Unable to get proposal")
            .expect("Got None when expecting proposal");

        assert_eq!(proposal, fetched_proposal);

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

        let fetched_proposal = store
            .get_proposal("WBKLF-BBBBB")
            .expect("Unable to get proposal")
            .expect("Got None when expecting proposal");

        assert_eq!(updated_proposal, fetched_proposal);
    }

    /// Verify that a proposal can be upgraded to a circuit
    ///
    /// 1. Run sqlite migrations
    /// 2. Create DieselAdminServiceStore
    /// 3. Create a proposal
    /// 4. Add proposal to store
    /// 5. Fetch Proposal from store
    /// 6. Validate fetched proposal is the same as the proposal added
    /// 7. Call upgrade_proposal_to_circuit for the proposal
    /// 8. Fetch the new circuit and validate it is as expected
    #[test]
    fn test_upgrade_proposals() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselAdminServiceStore::new(pool);

        let proposal = create_proposal();

        store
            .add_proposal(proposal.clone())
            .expect("Unable to add circuit proposal");

        let fetched_proposal = store
            .get_proposal("WBKLF-BBBBB")
            .expect("Unable to get proposal")
            .expect("Got None when expecting proposal");

        assert_eq!(proposal, fetched_proposal);

        store
            .upgrade_proposal_to_circuit("WBKLF-BBBBB")
            .expect("Unable to add circuit proposal");

        assert!(store
            .get_proposal("WBKLF-BBBBB")
            .expect("Unable to get proposal")
            .is_none());

        let fetched_circuit = store
            .get_circuit("WBKLF-BBBBB")
            .expect("Unable to get circuit")
            .expect("Got None when expecting circuit");

        assert_eq!(create_circuit(), fetched_circuit);
    }

    /// Verify that a circuit can be added to the store correctly and then fetched from the store
    ///
    /// 1. Run sqlite migrations
    /// 2. Create DieselAdminServiceStore
    /// 3. Create a circuit and nodes
    /// 4. Add circuit and nodes to store
    /// 5. Fetch Circuit from store
    /// 6. Validate fetched circuit is the same as the circuit added
    /// 7. Fetch CircuitNode from store
    /// 8. Validate fetched node is the same as the node added
    #[test]
    fn test_add_get_circuit_and_nodes() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselAdminServiceStore::new(pool);

        let circuit = create_circuit();

        let nodes = create_nodes();

        store
            .add_circuit(circuit.clone(), nodes)
            .expect("Unable to add circuit");

        let fetched_circuit = store
            .get_circuit("WBKLF-BBBBB")
            .expect("Unable to get circuit")
            .expect("Got None when expecting circuit");

        let fetched_node = store
            .get_node("bubba-node-000")
            .expect("Unable to get node")
            .expect("Got None when expecting node");

        assert_eq!(circuit, fetched_circuit);
        assert_eq!(
            fetched_node,
            CircuitNodeBuilder::default()
                .with_node_id("bubba-node-000".into())
                .with_endpoints(&vec!["tcps://splinterd-node-bubba:8044".into()])
                .build()
                .expect("Unable to build node"),
        )
    }

    /// Verify that list_circuits works correctly
    ///
    /// 1. Run sqlite migrations
    /// 2. Create DieselAdminServiceStore
    /// 3. Create a circuit and nodes
    /// 4. Add circuit to store
    /// 5. List circuits from store with no predicates, validate added circuit is returned
    /// 6. List circuits from store with management type predicate, validate added circuit is
    ///    returned
    /// 7. List circuits from store with member predicate, validate added circuit is
    ///    returned
    /// 8. List circuits from store with mismatching management type predicate, validate no
    ///    circuits are returned
    #[test]
    fn test_list_circuits() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselAdminServiceStore::new(pool);

        let circuit = create_circuit();
        let nodes = create_nodes();

        let extra_circuit = create_extra_circuit();
        let extra_nodes = create_extra_nodes();

        store
            .add_circuit(circuit.clone(), nodes)
            .expect("Unable to add circuit");

        // test no predicates
        let mut circuits = store
            .list_circuits(&vec![])
            .expect("Unable to list circuits");

        assert_eq!(circuits.next(), Some(circuit.clone()));
        assert_eq!(circuits.next(), None);

        // test management type predicate
        let mut circuits = store
            .list_circuits(&vec![CircuitPredicate::ManagementTypeEq(
                "gameroom".to_string(),
            )])
            .expect("Unable to list circuits with management type predicate");

        assert_eq!(circuits.next(), Some(circuit.clone()));
        assert_eq!(circuits.next(), None);

        // test bad management type predicate
        let mut circuits = store
            .list_circuits(&vec![CircuitPredicate::ManagementTypeEq(
                "arcade".to_string(),
            )])
            .expect("Unable to list circuits with management type predicate");

        assert_eq!(circuits.next(), None);

        store
            .add_circuit(extra_circuit.clone(), extra_nodes)
            .expect("Unable to add circuit");

        // test members type predicate
        let mut circuits = store
            .list_circuits(&vec![CircuitPredicate::MembersInclude(vec![
                "gumbo-node-000".to_string(),
            ])])
            .expect("Unable to list circuits with members include predicate");

        assert_eq!(circuits.next(), Some(extra_circuit));
        assert_eq!(circuits.next(), None);

        // show all circuits are returned
        let circuits = store
            .list_circuits(&vec![])
            .expect("Unable to list circuits with members include predicate");

        assert_eq!(circuits.len(), 2);
    }

    /// Verify that a circuit can be removed from the store
    ///
    /// 1. Run sqlite migrations
    /// 2. Create DieselAdminServiceStore
    /// 3. Create a circuit
    /// 4. Add circuit to store
    /// 5. Fetch circuit from store
    /// 6. Validate fetched circuit is the same as the proposal added
    /// 7. Remove circuit
    /// 8. Validate the circuit was removed
    #[test]
    fn test_remove_circuits() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselAdminServiceStore::new(pool);

        let circuit = create_circuit();
        let nodes = create_nodes();

        store
            .add_circuit(circuit.clone(), nodes)
            .expect("Unable to add circuit");

        let fetched_circuit = store
            .get_circuit("WBKLF-BBBBB")
            .expect("Unable to get circuit")
            .expect("Got None when expecting circuit");

        assert_eq!(circuit, fetched_circuit);

        store
            .remove_circuit("WBKLF-BBBBB")
            .expect("Unable to add circuit");

        let fetched_circuit = store
            .get_circuit("WBKLF-BBBBB")
            .expect("Unable to get circuit");

        assert_eq!(None, fetched_circuit);
    }

    /// Verify that a service can be fetched from the store
    ///
    /// 1. Run sqlite migrations
    /// 2. Create DieselAdminServiceStore
    /// 3. Create a circuit
    /// 4. Add circuit to store
    /// 5. Fetch circuit from store
    /// 6. fetch a service from the store
    #[test]
    fn test_get_service() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselAdminServiceStore::new(pool);

        let circuit = create_circuit();
        let nodes = create_nodes();

        store
            .add_circuit(circuit.clone(), nodes)
            .expect("Unable to add circuit");

        let fetched_circuit = store
            .get_circuit("WBKLF-BBBBB")
            .expect("Unable to get circuit")
            .expect("Got None when expecting circuit");

        assert_eq!(circuit, fetched_circuit);

        let service_id = ServiceId::new("WBKLF-BBBBB".to_string(), "a000".to_string());
        let fetched_service = store
            .get_service(&service_id)
            .expect("Unable to get service")
            .expect("Got None when expecting service");

        assert_eq!(fetched_circuit.roster()[0], fetched_service);
    }

    /// Verify that all service from a circuit can be listed from the store
    ///
    /// 1. Run sqlite migrations
    /// 2. Create DieselAdminServiceStore
    /// 3. Create a circuit
    /// 4. Add circuit to store
    /// 5. Fetch circuit from store
    /// 6. List all service from the circuit
    #[test]
    fn test_list_service() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselAdminServiceStore::new(pool);

        let circuit = create_circuit();
        let nodes = create_nodes();

        store
            .add_circuit(circuit.clone(), nodes)
            .expect("Unable to add circuit");

        let fetched_circuit = store
            .get_circuit("WBKLF-BBBBB")
            .expect("Unable to get circuit")
            .expect("Got None when expecting circuit");

        assert_eq!(circuit, fetched_circuit);

        let mut services = store
            .list_services("WBKLF-BBBBB")
            .expect("Unable to get services");

        assert!(fetched_circuit
            .roster()
            .contains(&services.next().expect("Unable to get service")));

        assert!(fetched_circuit
            .roster()
            .contains(&services.next().expect("Unable to get service")));

        assert_eq!(None, services.next());
    }

    /// Verify that all nodes can be listed from the store
    ///
    /// 1. Run sqlite migrations
    /// 2. Create DieselAdminServiceStore
    /// 3. Create a circuit and nodes
    /// 4. Add circuit and nodes to store
    /// 5. Fetch circuit from store
    /// 6. List all nodes from the store
    #[test]
    fn test_list_nodes() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselAdminServiceStore::new(pool);

        let circuit = create_circuit();
        let nodes = create_nodes();

        store
            .add_circuit(circuit.clone(), nodes)
            .expect("Unable to add circuit");

        let fetched_circuit = store
            .get_circuit("WBKLF-BBBBB")
            .expect("Unable to get circuit")
            .expect("Got None when expecting circuit");

        assert_eq!(circuit, fetched_circuit);

        let mut nodes = store.list_nodes().expect("Unable to get services");

        assert!(fetched_circuit.members().contains(
            &nodes
                .next()
                .expect("Unable to get service")
                .node_id()
                .to_string()
        ));

        assert!(fetched_circuit.members().contains(
            &nodes
                .next()
                .expect("Unable to get service")
                .node_id()
                .to_string()
        ));

        assert!(nodes.next().is_none());
    }

    /// Creates a connection pool for an in-memory SQLite database with only a single connection
    /// available. Each connection is backed by a different in-memory SQLite database, so limiting
    /// the pool to a single connection ensures that the same DB is used for all operations.
    fn create_connection_pool_and_migrate() -> Pool<ConnectionManager<SqliteConnection>> {
        let connection_manager = ConnectionManager::<SqliteConnection>::new(":memory:");
        let pool = Pool::builder()
            .max_size(1)
            .build(connection_manager)
            .expect("Failed to build connection pool");

        run_sqlite_migrations(&*pool.get().expect("Failed to get connection for migrations"))
            .expect("Failed to run migrations");

        pool
    }

    fn create_proposal() -> CircuitProposal {
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
                    .with_application_metadata(b"test")
                    .with_comments("This is a test")
                    .with_circuit_management_type("gameroom")
                    .build().expect("Unable to build circuit")
            )
            .with_requester(
                &parse_hex(
                    "0283a14e0a17cb7f665311e9b5560f4cde2b502f17e2d03223e15d90d9318d7482").unwrap())
            .with_requester_node_id("acme-node-000")
            .build().expect("Unable to build proposals")
    }

    fn create_extra_proposal() -> CircuitProposal {
        CircuitProposalBuilder::default()
            .with_proposal_type(&ProposalType::Create)
            .with_circuit_id("WBKLF-AAAAA")
            .with_circuit_hash(
                "7ddc426972710adc0b2ecd49e89a9dd805fb9206bf516079724c887bedbcdf1d")
            .with_circuit(
                &ProposedCircuitBuilder::default()
                    .with_circuit_id("WBKLF-AAAAA")
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
                            .with_node_id(&"gumbo-node-000")
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
                            .with_node_id("gumbo-node-000".into())
                            .with_endpoints(&vec!["tcps://splinterd-node-gumbo:8044".into()])
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

    fn create_circuit() -> Circuit {
        CircuitBuilder::default()
            .with_circuit_id("WBKLF-BBBBB")
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

    fn create_extra_circuit() -> Circuit {
        CircuitBuilder::default()
            .with_circuit_id("WBKLF-CCCCC")
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
                    .with_node_id("gumbo-node-000")
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
            .with_members(&vec!["gumbo-node-000".into(), "acme-node-000".into()])
            .with_circuit_management_type("other")
            .build()
            .expect("Unable to build circuit")
    }

    fn create_nodes() -> Vec<CircuitNode> {
        vec![
            CircuitNodeBuilder::default()
                .with_node_id("bubba-node-000".into())
                .with_endpoints(&vec!["tcps://splinterd-node-bubba:8044".into()])
                .build()
                .expect("Unable to build node"),
            CircuitNodeBuilder::default()
                .with_node_id("acme-node-000".into())
                .with_endpoints(&vec!["tcps://splinterd-node-acme:8044".into()])
                .build()
                .expect("Unable to build node"),
        ]
    }

    fn create_extra_nodes() -> Vec<CircuitNode> {
        vec![
            CircuitNodeBuilder::default()
                .with_node_id("gumbo-node-000".into())
                .with_endpoints(&vec!["tcps://splinterd-node-gumbo:8044".into()])
                .build()
                .expect("Unable to build node"),
            CircuitNodeBuilder::default()
                .with_node_id("acme-node-000".into())
                .with_endpoints(&vec!["tcps://splinterd-node-acme:8044".into()])
                .build()
                .expect("Unable to build node"),
        ]
    }
}
