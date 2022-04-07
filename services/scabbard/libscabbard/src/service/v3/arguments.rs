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

use splinter::error::InvalidArgumentError;
use splinter::service::ServiceId;

pub struct ScabbardArguments {
    peers: Vec<ServiceId>,
}

impl ScabbardArguments {
    pub fn new(peers: Vec<ServiceId>) -> Result<Self, InvalidArgumentError> {
        Ok(Self { peers })
    }

    pub fn peers(&self) -> &Vec<ServiceId> {
        &self.peers
    }
}

#[derive(Default)]
pub struct ScabbardArgumentsBuilder {
    peers: Option<Vec<ServiceId>>,
}

impl ScabbardArgumentsBuilder {
    pub fn new() -> Self {
        Self { peers: None }
    }

    pub fn with_peers(mut self, peers: Vec<ServiceId>) -> Self {
        self.peers = Some(peers);
        self
    }

    pub fn build(self) -> Result<ScabbardArguments, InvalidArgumentError> {
        let peers = self
            .peers
            .ok_or_else(|| InvalidArgumentError::new("peers", "must be set"))?;

        ScabbardArguments::new(peers)
    }
}
