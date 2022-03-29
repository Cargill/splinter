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

use std::collections::HashMap;

use crate::error::InternalError;
use crate::orchestrator::{ServiceDefinition, ServiceOrchestrator};

use super::LifecycleDispatch;

impl LifecycleDispatch for ServiceOrchestrator {
    fn add_service(
        &self,
        circuit_id: &str,
        service_id: &str,
        service_type: &str,
        args: Vec<(String, String)>,
    ) -> Result<(), InternalError> {
        if !self
            .supported_service_types()
            .contains(&service_type.to_string())
        {
            trace!(
                "Ignoring call to add service, service type not supported: {}",
                service_type
            );
            return Ok(());
        }

        debug!(
            "Adding service: {}::{} ({})",
            circuit_id, service_id, service_type,
        );

        let service_defination = ServiceDefinition {
            circuit: circuit_id.to_string(),
            service_id: service_id.to_string(),
            service_type: service_type.to_string(),
        };

        let mut arg_map = HashMap::new();
        for (key, value) in args {
            arg_map.insert(key, value);
        }

        ServiceOrchestrator::initialize_service(self, service_defination, arg_map)
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }

    fn retire_service(
        &self,
        circuit_id: &str,
        service_id: &str,
        service_type: &str,
    ) -> Result<(), InternalError> {
        if !self
            .supported_service_types()
            .contains(&service_type.to_string())
        {
            trace!(
                "Ignoring call to add service, service type not supported: {}",
                service_type
            );
            return Ok(());
        }

        debug!(
            "Retire service: {}::{} ({})",
            circuit_id, service_id, service_type,
        );

        let service_defination = ServiceDefinition {
            circuit: circuit_id.to_string(),
            service_id: service_id.to_string(),
            service_type: service_type.to_string(),
        };

        ServiceOrchestrator::stop_service(self, &service_defination)
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }

    fn purge_service(
        &self,
        circuit_id: &str,
        service_id: &str,
        service_type: &str,
    ) -> Result<(), InternalError> {
        if !self
            .supported_service_types()
            .contains(&service_type.to_string())
        {
            trace!(
                "Ignoring call to add service, service type not supported: {}",
                service_type
            );
            return Ok(());
        }

        debug!(
            "Purge service: {}::{} ({})",
            circuit_id, service_id, service_type,
        );

        let service_defination = ServiceDefinition {
            circuit: circuit_id.to_string(),
            service_id: service_id.to_string(),
            service_type: service_type.to_string(),
        };

        ServiceOrchestrator::purge_service(self, &service_defination)
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }

    fn shutdown_all_services(&self) -> Result<(), InternalError> {
        debug!("Shutdown all services");
        ServiceOrchestrator::shutdown_all_services(self)
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }

    fn add_stopped_service(
        &self,
        circuit_id: &str,
        service_id: &str,
        service_type: &str,
        args: HashMap<String, String>,
    ) -> Result<(), InternalError> {
        if !self
            .supported_service_types()
            .contains(&service_type.to_string())
        {
            trace!(
                "Ignoring call to add service, service type not supported: {}",
                service_type
            );
            return Ok(());
        }

        debug!(
            "Add stopped service: {}::{} ({})",
            circuit_id, service_id, service_type,
        );

        let service_defination = ServiceDefinition {
            circuit: circuit_id.to_string(),
            service_id: service_id.to_string(),
            service_type: service_type.to_string(),
        };

        let mut arg_map = HashMap::new();
        for (key, value) in args {
            arg_map.insert(key, value);
        }

        ServiceOrchestrator::add_stopped_service(self, service_defination, arg_map)
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }
}
