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

//! Builder for the BiomeSubsystem

use splinter::biome::UserProfileStore;
use splinter::error::InternalError;

use crate::node::runnable::biome::RunnableBiomeSubsystem;

pub struct BiomeSubsystemBuilder {
    profile_store: Option<Box<dyn UserProfileStore>>,
}

impl BiomeSubsystemBuilder {
    pub fn new() -> Self {
        Self {
            profile_store: None,
        }
    }

    /// Specifies the store factory to use with the node. Defaults to the MemoryStoreFactory.
    pub fn with_profile_store(mut self, profile_store: Box<dyn UserProfileStore>) -> Self {
        self.profile_store = Some(profile_store);
        self
    }

    pub fn build(self) -> Result<RunnableBiomeSubsystem, InternalError> {
        let profile_store = self.profile_store.ok_or_else(|| {
            InternalError::with_message(
                "Cannot build BiomeSubsystem without a store factory".to_string(),
            )
        })?;

        Ok(RunnableBiomeSubsystem { profile_store })
    }
}
