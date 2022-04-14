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

use std::convert::TryFrom;

use splinter::error::InvalidArgumentError;
use splinter::service::ServiceId;

pub struct ScabbardArguments {
    peers: Vec<ServiceId>,
    consensus: ScabbardConsensus,
}

impl ScabbardArguments {
    pub fn new(
        peers: Vec<ServiceId>,
        consensus: ScabbardConsensus,
    ) -> Result<Self, InvalidArgumentError> {
        Ok(Self { peers, consensus })
    }

    pub fn peers(&self) -> &Vec<ServiceId> {
        &self.peers
    }

    pub fn consensus(&self) -> &ScabbardConsensus {
        &self.consensus
    }
}

#[derive(Default)]
pub struct ScabbardArgumentsBuilder {
    peers: Option<Vec<ServiceId>>,
    consensus: Option<ScabbardConsensus>,
}

impl ScabbardArgumentsBuilder {
    pub fn new() -> Self {
        Self {
            peers: None,
            consensus: None,
        }
    }

    pub fn with_peers(mut self, peers: Vec<ServiceId>) -> Self {
        self.peers = Some(peers);
        self
    }

    pub fn with_consensus(mut self, consensus: ScabbardConsensus) -> Self {
        self.consensus = Some(consensus);
        self
    }

    pub fn build(self) -> Result<ScabbardArguments, InvalidArgumentError> {
        let peers = self
            .peers
            .ok_or_else(|| InvalidArgumentError::new("peers", "must be set"))?;

        // currently defaults to TwoPC if none is provided
        let consensus = self.consensus.unwrap_or(ScabbardConsensus::TwoPC);

        ScabbardArguments::new(peers, consensus)
    }
}

pub enum ScabbardConsensus {
    TwoPC,
}

impl TryFrom<String> for ScabbardConsensus {
    type Error = InvalidArgumentError;

    fn try_from(consensus: String) -> Result<Self, Self::Error> {
        match consensus.as_str() {
            "2PC" => Ok(ScabbardConsensus::TwoPC),
            _ => Err(InvalidArgumentError::new(
                "consensus",
                "provided consensus type is not supported",
            )),
        }
    }
}
