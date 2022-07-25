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

use std::time::Duration;

use splinter::{error::InvalidArgumentError, service::ServiceId};

const DEFAULT_JITTER: u64 = 5;
const DEFAULT_FREQUENCY: u64 = 10;
const DEFAULT_ERROR_RATE: f32 = 0.1;

pub struct EchoArguments {
    peers: Vec<ServiceId>,
    frequency: Duration,
    jitter: Duration,
    error_rate: f32,
}

impl EchoArguments {
    pub fn new(
        peers: Vec<ServiceId>,
        frequency: Duration,
        jitter: Duration,
        error_rate: f32,
    ) -> Result<Self, InvalidArgumentError> {
        Ok(EchoArguments {
            peers,
            frequency,
            jitter,
            error_rate,
        })
    }

    pub fn peers(&self) -> &Vec<ServiceId> {
        &self.peers
    }

    pub fn frequency(&self) -> &Duration {
        &self.frequency
    }

    pub fn jitter(&self) -> &Duration {
        &self.jitter
    }

    pub fn error_rate(&self) -> f32 {
        self.error_rate
    }
}

#[derive(Default)]
pub struct EchoArgumentsBuilder {
    peers: Option<Vec<ServiceId>>,
    frequency: Option<Duration>,
    jitter: Option<Duration>,
    error_rate: Option<f32>,
}

impl EchoArgumentsBuilder {
    pub fn new() -> Self {
        EchoArgumentsBuilder {
            peers: None,
            frequency: None,
            jitter: None,
            error_rate: None,
        }
    }

    pub fn with_peers(mut self, peers: Vec<ServiceId>) -> Self {
        self.peers = Some(peers);
        self
    }

    pub fn with_frequency(mut self, frequency: Duration) -> Self {
        self.frequency = Some(frequency);
        self
    }

    pub fn with_jitter(mut self, jitter: Duration) -> Self {
        self.jitter = Some(jitter);
        self
    }

    pub fn with_error_rate(mut self, error_rate: f32) -> Self {
        self.error_rate = Some(error_rate);
        self
    }

    pub fn build(self) -> Result<EchoArguments, InvalidArgumentError> {
        let peers = self
            .peers
            .ok_or_else(|| InvalidArgumentError::new("peers", "must be set"))?;

        let frequency = self
            .frequency
            .unwrap_or(Duration::from_secs(DEFAULT_FREQUENCY));

        let jitter = self.jitter.unwrap_or(Duration::from_secs(DEFAULT_JITTER));

        let error_rate = self.error_rate.unwrap_or(DEFAULT_ERROR_RATE);

        Ok(EchoArguments {
            peers,
            frequency,
            jitter,
            error_rate,
        })
    }
}
