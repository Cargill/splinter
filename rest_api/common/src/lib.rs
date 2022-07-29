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

pub mod auth;
pub mod bind_config;
pub mod error;
#[cfg(feature = "oauth")]
pub mod oauth_config;
pub mod paging;
#[cfg(feature = "registry")]
pub mod percent_encode_filter_query;
pub mod response_models;
#[cfg(feature = "scabbard")]
pub mod scabbard;
pub mod secrets;
pub mod sessions;
pub mod status;

pub const SPLINTER_PROTOCOL_VERSION: u32 = 2;
