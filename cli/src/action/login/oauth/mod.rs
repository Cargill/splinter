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

mod callback;
mod error;
mod provider;

use std::path::PathBuf;

/// Contains the user information returned by an OAuth2 Provider.
pub struct UserTokens {
    pub provider_type: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
}

impl std::fmt::Debug for UserTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("UserTokens")
            .field("provider_type", &self.provider_type)
            .field("access_token", &"<Redacted>".to_string())
            .field(
                "refresh_token",
                &self.refresh_token.as_deref().map(|_| "<Redacted>"),
            )
            .finish()
    }
}
