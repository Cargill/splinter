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

use crate::oauth::UserInfo;

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: String,
}

/// Serializes the given user information as a query string to pass to the client
pub fn user_info_to_query_string(user_info: &UserInfo) -> String {
    let mut query_string = format!("access_token=OAuth2:{}", user_info.access_token());
    query_string.push_str(&format!("&display_name={}", user_info.identity()));
    if let Some(duration) = user_info.expires_in() {
        query_string.push_str(&format!("&expires_in={}", duration.as_secs()))
    };
    if let Some(refresh) = user_info.refresh_token() {
        query_string.push_str(&format!("&refresh_token={}", refresh))
    };
    query_string
}
