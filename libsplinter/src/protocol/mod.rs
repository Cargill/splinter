// Copyright 2019 Cargill Incorporated
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

//! Protocol versions for various endpoints provided by splinter.

pub mod authorization;

pub const ADMIN_PROTOCOL_VERSION: u32 = 1;

#[cfg(feature = "rest-api-actix")]
pub(crate) const ADMIN_APPLICATION_REGISTRATION_PROTOCOL_MIN: u32 = 1;
pub(crate) const ADMIN_SERVICE_PROTOCOL_MIN: u32 = 1;
#[cfg(feature = "rest-api-actix")]
pub(crate) const ADMIN_SUBMIT_PROTOCOL_MIN: u32 = 1;

#[cfg(feature = "rest-api-actix")]
pub(crate) const ADMIN_FETCH_PROPOSALS_PROTOCOL_MIN: u32 = 1;

#[cfg(feature = "rest-api-actix")]
pub(crate) const ADMIN_LIST_PROPOSALS_PROTOCOL_MIN: u32 = 1;
#[cfg(feature = "rest-api-actix")]
pub(crate) const ADMIN_LIST_CIRCUITS_MIN: u32 = 1;
#[cfg(feature = "rest-api-actix")]
pub(crate) const ADMIN_FETCH_CIRCUIT_MIN: u32 = 1;

pub const REGISTRY_PROTOCOL_VERSION: u32 = 1;

#[cfg(feature = "rest-api-actix")]
pub(crate) const REGISTRY_LIST_NODES_MIN: u32 = 1;
#[cfg(feature = "rest-api-actix")]
pub(crate) const REGISTRY_FETCH_NODE_MIN: u32 = 1;

#[cfg(feature = "biome")]
pub const BIOME_PROTOCOL_VERSION: u32 = 1;

#[cfg(all(feature = "biome-credentials", feature = "rest-api",))]
pub(crate) const BIOME_REGISTER_PROTOCOL_MIN: u32 = 1;
#[cfg(all(feature = "biome-credentials", feature = "rest-api",))]
pub(crate) const BIOME_LOGIN_PROTOCOL_MIN: u32 = 1;
#[cfg(all(feature = "biome-credentials", feature = "rest-api",))]
pub(crate) const BIOME_USER_PROTOCOL_MIN: u32 = 1;
#[cfg(all(feature = "biome-credentials", feature = "rest-api",))]
pub(crate) const BIOME_LIST_USERS_PROTOCOL_MIN: u32 = 1;
#[cfg(all(feature = "biome-credentials", feature = "rest-api"))]
pub(crate) const BIOME_VERIFY_PROTOCOL_MIN: u32 = 1;

#[cfg(all(feature = "biome-key-management", feature = "rest-api",))]
pub(crate) const BIOME_KEYS_PROTOCOL_MIN: u32 = 1;
