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

use crate::service::EchoArguments;
use crate::store::EchoStoreFactory;

pub struct EchoPrepareServiceCommand<C> {
    store_factory: Arc<dyn EchoStoreFactory<C>>,
    service: FullyQualifiedServiceId,
    arguments: EchoArguments,
}

impl<C> EchoPrepareServiceCommand<C> {
    pub fn new(
        store_factory: Arc<dyn EchoStoreFactory<C>>,
        service: FullyQualifiedServiceId,
        arguments: EchoArguments,
    ) -> Self {
        EchoPrepareServiceCommand {
            store_factory,
            service,
            arguments,
        }
    }
}

impl<C> StoreCommand for EchoPrepareServiceCommand<C> {
    type Context = C;

    fn execute(&self, conn: &Self::Context) -> Result<(), InternalError> {
        self.store_factory
            .new_store(conn)
            .add_service(&self.service, &self.arguments)
    }
}
