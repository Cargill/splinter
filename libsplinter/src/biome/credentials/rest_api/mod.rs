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

mod actix_web_1;
mod resources;

#[cfg(feature = "authorization")]
use crate::rest_api::auth::authorization::Permission;

pub use actix_web_1::{
    BiomeCredentialsRestConfig, BiomeCredentialsRestConfigBuilder,
    BiomeCredentialsRestResourceProvider, BiomeCredentialsRestResourceProviderBuilder,
};

#[cfg(feature = "authorization")]
const BIOME_USER_READ_PERMISSION: Permission = Permission::Check {
    permission_id: "biome.user.read",
    permission_display_name: "Biome user read",
    permission_description: "Allows the client to view all Biome users",
};
#[cfg(feature = "authorization")]
const BIOME_USER_WRITE_PERMISSION: Permission = Permission::Check {
    permission_id: "biome.user.write",
    permission_display_name: "Biome user write",
    permission_description: "Allows the client to modify all Biome users",
};
