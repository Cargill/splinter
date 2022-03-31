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

use std::marker::PhantomData;

use log::info;
use splinter::{error::InternalError, store::command::StoreCommand};

#[derive(Default)]
pub struct ScabbardFinalizeServiceCommand<C> {
    _store_factory: PhantomData<C>,
}

impl<C> ScabbardFinalizeServiceCommand<C> {
    pub fn new() -> Self {
        Self {
            _store_factory: PhantomData,
        }
    }
}

impl<C> StoreCommand for ScabbardFinalizeServiceCommand<C> {
    type Context = C;

    fn execute(&self, _conn: &Self::Context) -> Result<(), InternalError> {
        info!("executing finalize service command");
        Ok(())
    }
}
