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

use std::sync::Arc;

use splinter::{error::InternalError, service::ServiceId, store::command::StoreCommand};

use crate::store::{
    context::ConsensusContext,
    service::ScabbardService,
    two_phase::{Context, ContextBuilder, Participant, State},
    ScabbardStoreFactory,
};

pub struct ScabbardPrepareServiceCommand<C> {
    store_factory: Arc<dyn ScabbardStoreFactory<C>>,
    scabbard_service: ScabbardService,
}

impl<C> ScabbardPrepareServiceCommand<C> {
    pub fn new(
        store_factory: Arc<dyn ScabbardStoreFactory<C>>,
        scabbard_service: ScabbardService,
    ) -> Self {
        Self {
            store_factory,
            scabbard_service,
        }
    }
}

impl<C> StoreCommand for ScabbardPrepareServiceCommand<C> {
    type Context = C;

    fn execute(&self, conn: &Self::Context) -> Result<(), InternalError> {
        let context = create_context(&self.scabbard_service)?;

        let store = self.store_factory.new_store(conn);

        store
            .add_service(self.scabbard_service.clone())
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        store
            .add_consensus_context(
                self.scabbard_service.service_id(),
                ConsensusContext::TwoPhaseCommit(context),
            )
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        Ok(())
    }
}

fn create_context(service: &ScabbardService) -> Result<Context, InternalError> {
    let mut peers = service.peers().to_vec();

    peers.push(service.service_id().service_id().clone());

    let coordinator = get_coordinator(peers).ok_or_else(|| {
        InternalError::with_message(format!(
            "Unable to get coordinator service ID for service {}",
            service.service_id()
        ))
    })?;

    let state = if service.service_id().service_id() == &coordinator {
        State::WaitingForStart
    } else {
        State::WaitingForVoteRequest
    };
    ContextBuilder::default()
        .with_coordinator(&coordinator)
        .with_epoch(1)
        .with_participants(
            service
                .peers()
                .iter()
                .map(|participant| Participant {
                    process: participant.clone(),
                    vote: None,
                })
                .collect(),
        )
        .with_state(state)
        .with_this_process(service.service_id().clone().service_id())
        .build()
        .map_err(|err| InternalError::from_source(Box::new(err)))
}

/// Gets the ID of the coordinator. The coordinator is the node with the lowest ID in the set of
/// verifiers.
fn get_coordinator(peers: Vec<ServiceId>) -> Option<ServiceId> {
    peers.into_iter().min_by(|x, y| x.as_str().cmp(y.as_str()))
}
