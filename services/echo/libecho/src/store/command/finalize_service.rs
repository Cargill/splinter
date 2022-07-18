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

use crate::service::EchoServiceStatus;
use crate::store::EchoStoreFactory;

pub struct EchoFinalizeServiceCommand<C> {
    store_factory: Arc<dyn EchoStoreFactory<C>>,
    service: FullyQualifiedServiceId,
}

impl<C> EchoFinalizeServiceCommand<C> {
    pub fn new(
        store_factory: Arc<dyn EchoStoreFactory<C>>,
        service: FullyQualifiedServiceId,
    ) -> Self {
        EchoFinalizeServiceCommand {
            store_factory,
            service,
        }
    }
}

impl<C> StoreCommand for EchoFinalizeServiceCommand<C> {
    type Context = C;

    // uses the `update_service_status` store method to set the service status to
    // `EchoServiceStatus::Prepared`
    fn execute(&self, conn: &Self::Context) -> Result<(), InternalError> {
        self.store_factory
            .new_store(conn)
            .update_service_status(&self.service, EchoServiceStatus::Finalized)
    }
}
