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

use crate::auth::oauth::UserTokens;

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: String,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct CallbackResponse<'a> {
    pub access_token: &'a str,
    pub expires_in: Option<u64>,
    pub refresh_token: Option<&'a str>,
}

impl<'a> From<&'a UserTokens> for CallbackResponse<'a> {
    fn from(tokens: &'a UserTokens) -> Self {
        Self {
            access_token: tokens.access_token(),
            expires_in: tokens.expires_in().map(|duration| duration.as_secs()),
            refresh_token: tokens.refresh_token(),
        }
    }
}
