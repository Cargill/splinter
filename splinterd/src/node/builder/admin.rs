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

//! Builder for the AdminSubsystem

use std::time::Duration;

use rand::{thread_rng, Rng};
use splinter::error::InternalError;
use splinter::store::{memory::MemoryStoreFactory, StoreFactory};
use splinter::transport::multi::MultiTransport;

use crate::node::runnable::admin::RunnableAdminSubsystem;

const DEFAULT_ADMIN_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct AdminSubsystemBuilder {
    node_id: Option<String>,
    admin_timeout: Option<Duration>,
    heartbeat_interval: Option<Duration>,
    strict_ref_counts: bool,
    store_factory: Option<Box<dyn StoreFactory>>,
}

impl AdminSubsystemBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Specifies the id for the node. Defaults to a random node id.
    pub fn with_node_id(mut self, node_id: String) -> Self {
        self.node_id = Some(node_id);
        self
    }

    /// Specifies the timeout for admin requests. Defaults to 30 seconds.
    pub fn with_admin_timeout(mut self, admin_timeout: Duration) -> Self {
        self.admin_timeout = Some(admin_timeout);
        self
    }

    /// Specifies the heartbeat interval between peer connections. Defaults to 30 seconds.
    pub fn with_heartbeat_interval(mut self, heartbeat_interval: Duration) -> Self {
        self.heartbeat_interval = Some(heartbeat_interval);
        self
    }

    /// Configure whether or not strict reference counts will be used in the peer manager. Defaults
    /// to false.
    pub fn with_strict_ref_counts(mut self, strict_ref_counts: bool) -> Self {
        self.strict_ref_counts = strict_ref_counts;
        self
    }

    /// Specifies the store factory to use with the node. Defaults to the MemoryStoreFactory.
    pub fn with_store_factory(mut self, store_factory: Box<dyn StoreFactory>) -> Self {
        self.store_factory = Some(store_factory);
        self
    }

    pub fn build(mut self) -> Result<RunnableAdminSubsystem, InternalError> {
        let node_id = self
            .node_id
            .take()
            .unwrap_or_else(|| format!("n{}", thread_rng().gen::<u16>().to_string()));

        let admin_timeout = self.admin_timeout.unwrap_or(DEFAULT_ADMIN_TIMEOUT);
        let heartbeat_interval = self
            .heartbeat_interval
            .take()
            .unwrap_or(DEFAULT_HEARTBEAT_INTERVAL);

        let store_factory = self
            .store_factory
            .unwrap_or_else(|| Box::new(MemoryStoreFactory::new()));

        let transport = MultiTransport::new(vec![]);

        Ok(RunnableAdminSubsystem {
            node_id,
            transport,
            admin_timeout,
            heartbeat_interval,
            store_factory,
            strict_ref_counts: self.strict_ref_counts,
        })
    }
}
