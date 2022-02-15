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

use crate::circuit::routing::{RoutingTableReader, ServiceId as RoutingServiceId};
use crate::error::InternalError;
use crate::service::{FullyQualifiedServiceId, ServiceType};

use super::type_resolver::ServiceTypeResolver;

pub struct RoutingTableServiceTypeResolver {
    routing_table_reader: Box<dyn RoutingTableReader>,
}

impl RoutingTableServiceTypeResolver {
    pub fn new(routing_table_reader: Box<dyn RoutingTableReader>) -> Self {
        Self {
            routing_table_reader,
        }
    }
}

impl ServiceTypeResolver for RoutingTableServiceTypeResolver {
    fn resolve_type(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<ServiceType>, InternalError> {
        self.routing_table_reader
            .get_service(&RoutingServiceId::new(
                service_id.circuit_id().to_string(),
                service_id.service_id().to_string(),
            ))
            .map_err(|err| InternalError::from_source(Box::new(err)))?
            .map(|service| {
                ServiceType::new(service.service_type())
                    .map_err(|e| InternalError::from_source(Box::new(e)))
            })
            .transpose()
    }
}
