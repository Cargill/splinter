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

//! Provides [OAuthUserSessionStore] operations implemented for a diesel backend

pub(super) mod add_session;
pub(super) mod get_session;
pub(super) mod get_user;
pub(super) mod remove_session;
pub(super) mod update_session;

pub(super) struct OAuthUserSessionStoreOperations<'a, C> {
    conn: &'a C,
}

impl<'a, C> OAuthUserSessionStoreOperations<'a, C>
where
    C: diesel::Connection,
{
    pub fn new(conn: &'a C) -> Self {
        OAuthUserSessionStoreOperations { conn }
    }
}
