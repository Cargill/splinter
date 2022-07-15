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
