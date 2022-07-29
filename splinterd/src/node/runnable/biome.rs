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

//! Builder for configuring Biome REST API resources

use std::sync::Arc;

use splinter::biome::UserProfileStore;
use splinter::error::InternalError;
use splinter::store::StoreFactory;
use splinter_rest_api_actix_web_1::biome::key_management::BiomeKeyManagementRestResourceProvider;
use splinter_rest_api_actix_web_1::biome::{
    credentials::BiomeCredentialsRestResourceProviderBuilder,
    profile::BiomeProfileRestResourceProvider,
};
use splinter_rest_api_actix_web_1::{
    framework::AuthConfig, framework::Resource as Actix1Resource, framework::RestResourceProvider,
};

use crate::node::running::biome::BiomeSubsystem;

/// Biome resource provider
pub struct BiomeResourceProvider {
    /// Biome's ActixWeb1 resources
    pub(crate) actix1_resources: Vec<Actix1Resource>,
    /// Biome-specific authorization configurations
    pub(crate) auth_configs: Vec<AuthConfig>,
}

impl BiomeResourceProvider {
    /// Build a `BiomeResourceProvider`
    pub fn new(store_factory: &dyn StoreFactory) -> Result<BiomeResourceProvider, InternalError> {
        // Used to collect the Biome-specific Actix 1 resources
        let mut actix1_resources = vec![];

        // Create the `BiomeCredentialsRestResourceProvider` to create the credentials resources
        let mut credentials_resource_builder: BiomeCredentialsRestResourceProviderBuilder =
            Default::default();
        credentials_resource_builder = credentials_resource_builder
            .with_credentials_store(store_factory.get_biome_credentials_store())
            .with_refresh_token_store(store_factory.get_biome_refresh_token_store())
            .with_key_store(store_factory.get_biome_key_store());
        let credentials_resource_provider = credentials_resource_builder
            .build()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        // Create the `BiomeKeyManagementRestResourceProvider` to create the key-related resources
        let key_management_resource_provider = BiomeKeyManagementRestResourceProvider::new(
            Arc::new(store_factory.get_biome_key_store()),
        );

        // Create the `BiomeProfileRestResourceProvider` to create profile-related resources
        let profile_resource_provider = BiomeProfileRestResourceProvider::new(Arc::new(
            store_factory.get_biome_user_profile_store(),
        ));

        actix1_resources.extend(profile_resource_provider.resources());
        actix1_resources.extend(credentials_resource_provider.resources());
        actix1_resources.extend(key_management_resource_provider.resources());

        Ok(BiomeResourceProvider {
            actix1_resources,
            auth_configs: vec![AuthConfig::Biome {
                biome_credentials_resource_provider: credentials_resource_provider,
            }],
        })
    }

    /// Take the available REST Resources from the Biome resource provider.
    pub fn take_actix1_resources(&mut self) -> Vec<Actix1Resource> {
        let mut replaced = vec![];
        std::mem::swap(&mut self.actix1_resources, &mut replaced);
        replaced
    }
}

pub struct RunnableBiomeSubsystem {
    pub profile_store: Box<dyn UserProfileStore>,
}

impl RunnableBiomeSubsystem {
    pub fn run(self) -> Result<BiomeSubsystem, InternalError> {
        let profile_store = self.profile_store;
        Ok(BiomeSubsystem { profile_store })
    }
}
