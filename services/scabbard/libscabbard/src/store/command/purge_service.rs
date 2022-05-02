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

use splinter::{
    error::InternalError, service::FullyQualifiedServiceId, store::command::StoreCommand,
};

use crate::store::ScabbardStoreFactory;

pub struct ScabbardPurgeServiceCommand<C> {
    store_factory: Arc<dyn ScabbardStoreFactory<C>>,
    service_id: FullyQualifiedServiceId,
}

impl<C> ScabbardPurgeServiceCommand<C> {
    pub fn new(
        store_factory: Arc<dyn ScabbardStoreFactory<C>>,
        service_id: FullyQualifiedServiceId,
    ) -> Self {
        Self {
            store_factory,
            service_id,
        }
    }
}

impl<C> StoreCommand for ScabbardPurgeServiceCommand<C> {
    type Context = C;

    fn execute(&self, conn: &Self::Context) -> Result<(), InternalError> {
        self.store_factory
            .new_store(conn)
            .remove_service(&self.service_id)
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }
}
