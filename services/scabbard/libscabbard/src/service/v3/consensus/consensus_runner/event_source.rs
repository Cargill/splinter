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

use splinter::error::InternalError;
use splinter::service::FullyQualifiedServiceId;

use crate::store::ConsensusEvent;
use crate::store::Identified;

pub trait UnprocessedEventSource {
    /// Returns the next event for a given service that requires processing,
    /// if one exists.
    fn get_next_event(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<Identified<ConsensusEvent>>, InternalError>;
}
