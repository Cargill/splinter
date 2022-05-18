// Copyright 2018-2022 Cargill Incorporated
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

//! This module contains implementations of the consensus runner traits for a
//! `Box<dyn ScabbardStore>`

use splinter::error::InternalError;
use splinter::service::FullyQualifiedServiceId;

use crate::store::{ConsensusAction, ConsensusContext, ConsensusEvent, Identified, ScabbardStore};

use super::{ContextSource, UnprocessedActionSource, UnprocessedEventSource};

pub struct StoreUnprocessedEventSource {
    store: Box<dyn ScabbardStore>,
}

impl StoreUnprocessedEventSource {
    pub fn new(store: Box<dyn ScabbardStore>) -> Self {
        Self { store }
    }
}

impl UnprocessedEventSource for StoreUnprocessedEventSource {
    /// Returns the next event for a given service that requires processing,
    /// if one exists.
    fn get_next_event(
        &self,
        service_id: &FullyQualifiedServiceId,
        epoch: u64,
    ) -> Result<Option<Identified<ConsensusEvent>>, InternalError> {
        Ok(self
            .store
            .list_consensus_events(service_id, epoch)
            .map_err(|err| InternalError::from_source(Box::new(err)))?
            .get(0)
            .cloned())
    }
}

pub struct StoreUnprocessedActionSource {
    store: Box<dyn ScabbardStore>,
}

impl StoreUnprocessedActionSource {
    pub fn new(store: Box<dyn ScabbardStore>) -> Self {
        Self { store }
    }
}

impl UnprocessedActionSource for StoreUnprocessedActionSource {
    /// Returns actions for a given service that require processing.
    fn get_unprocessed_actions(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Vec<Identified<ConsensusAction>>, InternalError> {
        self.store
            .list_consensus_actions(service_id)
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }
}

pub struct StoreContextSource {
    store: Box<dyn ScabbardStore>,
}

impl StoreContextSource {
    pub fn new(store: Box<dyn ScabbardStore>) -> Self {
        Self { store }
    }
}

impl ContextSource for StoreContextSource {
    fn get_context(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<ConsensusContext>, InternalError> {
        self.store
            .get_current_consensus_context(service_id)
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }
}
