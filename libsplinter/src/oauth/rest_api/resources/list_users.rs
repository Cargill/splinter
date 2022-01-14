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

//! Defines OAuthUsers returned by the `OAuthResourceProvider`.

use crate::biome::oauth::store::OAuthUser;
use crate::rest_api::paging::{Paging, DEFAULT_LIMIT, DEFAULT_OFFSET};

#[derive(Serialize)]
pub(crate) struct ListOAuthUserResponse<'a> {
    pub data: Vec<OAuthUserResponse<'a>>,
    pub paging: Paging,
}

#[derive(Serialize)]
pub(crate) struct OAuthUserResponse<'a> {
    pub subject: &'a str,
    pub user_id: &'a str,
}

impl<'a> From<&'a OAuthUser> for OAuthUserResponse<'a> {
    fn from(user: &'a OAuthUser) -> Self {
        Self {
            subject: user.subject(),
            user_id: user.user_id(),
        }
    }
}

#[derive(Deserialize)]
pub struct PagingQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_offset")]
    pub offset: usize,
}

fn default_limit() -> usize {
    DEFAULT_LIMIT
}

fn default_offset() -> usize {
    DEFAULT_OFFSET
}
