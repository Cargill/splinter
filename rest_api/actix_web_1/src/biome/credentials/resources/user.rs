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

#[cfg(feature = "biome-key-management")]
use super::key_management::NewKey;

#[derive(Deserialize)]
pub struct ModifyUser {
    pub username: String,
    pub hashed_password: String,
    pub new_password: Option<String>,
    #[cfg(feature = "biome-key-management")]
    pub new_key_pairs: Vec<NewKey>,
}
