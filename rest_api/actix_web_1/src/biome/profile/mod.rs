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

mod profiles;
mod profiles_identity;
mod route;

use std::sync::Arc;

use splinter::biome::profile::store::UserProfileStore;

use crate::framework::{Resource, RestResourceProvider};
#[cfg(feature = "authorization")]
use splinter_rest_api_common::auth::Permission;

#[cfg(feature = "authorization")]
const BIOME_PROFILE_READ_PERMISSION: Permission = Permission::Check {
    permission_id: "biome.profile.read",
    permission_display_name: "Biome profile read",
    permission_description: "Allows the client to view all Biome user profiles",
};

/// Provides the following REST API endpoints for Biome profiles:
///
/// * `GET /biome/profile` - Get the profile information of the authenticated user
/// * `GET /biome/profiles` - Get a list of all user profiles
/// * `GET /biome/profiles/{id}` - Retrieve the profile with the specified ID
pub struct BiomeProfileRestResourceProvider {
    profile_store: Arc<dyn UserProfileStore>,
}

impl BiomeProfileRestResourceProvider {
    pub fn new(profile_store: Arc<dyn UserProfileStore>) -> Self {
        Self { profile_store }
    }
}

impl RestResourceProvider for BiomeProfileRestResourceProvider {
    fn resources(&self) -> Vec<Resource> {
        vec![
            profiles::make_profiles_list_route(self.profile_store.clone()),
            profiles_identity::make_profiles_routes(self.profile_store.clone()),
            route::make_profile_route(self.profile_store.clone()),
        ]
    }
}
