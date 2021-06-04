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

use serde::Deserialize;

use crate::action::api::SplinterRestClient;
use crate::error::CliError;

pub(super) const PAGING_LIMIT: &str = "1000";
// The Biome protocol version supported by the current CLI
pub(super) const CLI_SPLINTER_USER_PROTOCOL_VERSION: &str = "1";

impl SplinterRestClient {
    pub fn list_biome_users(&self) -> Result<Vec<ClientBiomeUser>, CliError> {
        unimplemented!();
    }

    /// Submits a request to list Biome's OAuth users
    pub fn list_oauth_users(&self) -> Result<ClientOAuthUserListResponse, CliError> {
        unimplemented!();
    }
}

/// Biome OAuth user details.
#[derive(Debug, Deserialize)]
pub struct ClientOAuthUser {
    pub subject: String,
    pub user_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ClientOAuthUserListResponse {
    pub data: Vec<ClientOAuthUser>,
    pub paging: Paging,
}

/// Biome user details, specific to the client to allow for deserializing the response data.
#[derive(Debug, Deserialize)]
pub struct ClientBiomeUser {
    pub username: String,
    pub user_id: String,
}

#[derive(Debug, Deserialize)]
pub struct Paging {
    pub current: String,
    pub offset: usize,
    pub limit: usize,
    pub total: usize,
    pub first: String,
    pub prev: String,
    pub next: String,
    pub last: String,
}
