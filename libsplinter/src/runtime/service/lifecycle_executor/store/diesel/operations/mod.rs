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

//! Provides database operations for the `DieselLifecycleStore`.

pub(super) mod add_service;
pub(super) mod get_service;
pub(super) mod list_service;
pub(super) mod remove_service;
pub(super) mod update_service;

pub struct LifecycleStoreOperations<'a, C> {
    conn: &'a C,
}

impl<'a, C: diesel::Connection> LifecycleStoreOperations<'a, C> {
    pub fn new(conn: &'a C) -> Self {
        LifecycleStoreOperations { conn }
    }
}
