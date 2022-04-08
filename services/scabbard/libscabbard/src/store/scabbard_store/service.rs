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

use std::fmt;

use splinter::error::InvalidStateError;
use splinter::service::{FullyQualifiedServiceId, ServiceId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScabbardService {
    service_id: FullyQualifiedServiceId,
    peers: Vec<ServiceId>,
    status: ServiceStatus,
}

#[allow(dead_code)]
impl ScabbardService {
    /// Returns the service ID for the scabbard service
    pub fn service_id(&self) -> &FullyQualifiedServiceId {
        &self.service_id
    }

    /// Returns the list of peers for the scabbard service
    pub fn peers(&self) -> &[ServiceId] {
        &self.peers
    }

    /// Returns the status of the scabbard service
    pub fn status(&self) -> &ServiceStatus {
        &self.status
    }
}

#[derive(Default, Clone)]
pub struct ScabbardServiceBuilder {
    service_id: Option<FullyQualifiedServiceId>,
    peers: Option<Vec<ServiceId>>,
    status: Option<ServiceStatus>,
}

#[allow(dead_code)]
impl ScabbardServiceBuilder {
    /// Returns the service ID for the service
    pub fn service_id(&self) -> Option<FullyQualifiedServiceId> {
        self.service_id.clone()
    }

    /// Returns the peers for the service
    pub fn peers(&self) -> Option<Vec<ServiceId>> {
        self.peers.clone()
    }

    /// Returns the peers for the service
    pub fn status(&self) -> Option<ServiceStatus> {
        self.status.clone()
    }

    /// Sets the service ID
    ///
    /// # Arguments
    ///
    ///  * `service_id` - The service ID for scabbard service
    pub fn with_service_id(
        mut self,
        service_id: &FullyQualifiedServiceId,
    ) -> ScabbardServiceBuilder {
        self.service_id = Some(service_id.clone());
        self
    }

    /// Sets the peers
    ///
    /// # Arguments
    ///
    ///  * `peers` - The peers for scabbard service
    pub fn with_peers(mut self, peers: &[ServiceId]) -> ScabbardServiceBuilder {
        self.peers = Some(peers.to_vec());
        self
    }

    /// Sets the status
    ///
    /// # Arguments
    ///
    ///  * `status` - The status for scabbard service
    pub fn with_status(mut self, status: &ServiceStatus) -> ScabbardServiceBuilder {
        self.status = Some(status.clone());
        self
    }

    /// Builds the `ScabbardService`
    ///
    /// Returns an error if the service ID, peers, or status is not set
    pub fn build(self) -> Result<ScabbardService, InvalidStateError> {
        let service_id = self.service_id.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `service_id`".to_string(),
            )
        })?;

        let peers = self.peers.ok_or_else(|| {
            InvalidStateError::with_message("unable to build, missing field: `peers`".to_string())
        })?;

        let status = self.status.ok_or_else(|| {
            InvalidStateError::with_message("unable to build, missing field: `status`".to_string())
        })?;

        Ok(ScabbardService {
            service_id,
            peers,
            status,
        })
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceStatus {
    Prepared,
    Finalized,
    Retired,
}

impl fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceStatus::Prepared => write!(f, "Status: Prepared"),
            ServiceStatus::Finalized => write!(f, "Status: Finalized"),
            ServiceStatus::Retired => write!(f, "Status: Retired"),
        }
    }
}
