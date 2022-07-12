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

//! Contains the publishing context for Scabbard v3

use cylinder::Signer;
use splinter::service::FullyQualifiedServiceId;

pub struct ScabbardPublishingContext {
    /// The `Signer` signing artifSignedTimestampCreator
    signer: Box<dyn Signer>,
    /// The service ID this context is for
    service_id: FullyQualifiedServiceId,
}

impl ScabbardPublishingContext {
    pub fn new(
        signer: Box<dyn Signer>,
        service_id: FullyQualifiedServiceId,
    ) -> ScabbardPublishingContext {
        ScabbardPublishingContext { signer, service_id }
    }

    pub fn signer(&self) -> &dyn Signer {
        &*self.signer
    }

    pub fn service_id(&self) -> &FullyQualifiedServiceId {
        &self.service_id
    }
}
