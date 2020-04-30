// Copyright 2018-2020 Cargill Incorporated
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

//! Service lookup implementations on the ServiceConnector.

use super::{ServiceLookup, ServiceLookupError, ServiceLookupProvider};
use crate::service::network::ServiceConnector;

impl ServiceLookup for ServiceConnector {
    fn connection_id(&self, service_id: &str) -> Result<Option<String>, ServiceLookupError> {
        self.get_connection_id(service_id)
            .map_err(|err| ServiceLookupError(err.to_string()))
    }

    fn service_id(&self, connection_id: &str) -> Result<Option<String>, ServiceLookupError> {
        self.get_identity(connection_id)
            .map_err(|err| ServiceLookupError(err.to_string()))
    }
}

impl ServiceLookupProvider for ServiceConnector {
    fn service_lookup(&self) -> Box<dyn ServiceLookup> {
        Box::new(self.clone())
    }
}
