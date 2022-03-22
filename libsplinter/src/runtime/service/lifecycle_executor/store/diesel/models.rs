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

use crate::error::InternalError;
use crate::runtime::service::{
    LifecycleCommand, LifecycleService, LifecycleStatus, LifecycleStoreError,
};

use super::schema::{service_lifecycle_argument, service_lifecycle_status};

/// Database model representation of `LifecycleService`
#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "service_lifecycle_status"]
#[primary_key(circuit_id, service_id)]
pub struct ServiceLifecycleStatusModel {
    pub circuit_id: String,
    pub service_id: String,
    pub service_type: String,
    pub command: String,
    pub status: String,
}

impl From<&LifecycleService> for ServiceLifecycleStatusModel {
    fn from(service: &LifecycleService) -> Self {
        ServiceLifecycleStatusModel {
            circuit_id: service.service_id().circuit_id().as_str().into(),
            service_id: service.service_id().service_id().as_str().into(),
            service_type: service.service_type().to_string(),
            command: service.command().into(),
            status: service.status().into(),
        }
    }
}

/// Database model representation of the arguments in a `LifecycleService`
#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "service_lifecycle_argument"]
#[primary_key(circuit_id, service_id, key)]
pub struct ServiceLifecycleArgumentModel {
    pub circuit_id: String,
    pub service_id: String,
    pub key: String,
    pub value: String,
    pub position: i32,
}

impl TryFrom<&LifecycleService> for Vec<ServiceLifecycleArgumentModel> {
    type Error = LifecycleStoreError;

    fn try_from(service: &LifecycleService) -> Result<Self, Self::Error> {
        let mut service_arguments = Vec::new();
        service_arguments.extend(
            service
                .arguments()
                .iter()
                .enumerate()
                .map(|(idx, (key, value))| {
                    Ok(ServiceLifecycleArgumentModel {
                        circuit_id: service.service_id().circuit_id().as_str().into(),
                        service_id: service.service_id().service_id().as_str().into(),
                        key: key.clone(),
                        value: value.clone(),
                        position: i32::try_from(idx).map_err(|_| {
                            LifecycleStoreError::Internal(InternalError::with_message(
                                "Unable to convert index into i32".to_string(),
                            ))
                        })?,
                    })
                })
                .collect::<Result<Vec<ServiceLifecycleArgumentModel>, LifecycleStoreError>>()?,
        );
        Ok(service_arguments)
    }
}

impl From<&LifecycleCommand> for String {
    fn from(command: &LifecycleCommand) -> Self {
        match *command {
            LifecycleCommand::Prepare => "PREPARE".into(),
            LifecycleCommand::Finalize => "FINALIZE".into(),
            LifecycleCommand::Retire => "RETIRE".into(),
            LifecycleCommand::Purge => "PURGE".into(),
        }
    }
}

impl TryFrom<&str> for LifecycleCommand {
    type Error = LifecycleStoreError;

    fn try_from(command: &str) -> Result<Self, Self::Error> {
        match command {
            "PREPARE" => Ok(LifecycleCommand::Prepare),
            "FINALIZE" => Ok(LifecycleCommand::Finalize),
            "RETIRE" => Ok(LifecycleCommand::Retire),
            "PURGE" => Ok(LifecycleCommand::Purge),
            _ => Err(LifecycleStoreError::Internal(InternalError::with_message(
                format!("Unknown command {}", command),
            ))),
        }
    }
}

impl TryFrom<&str> for LifecycleStatus {
    type Error = LifecycleStoreError;

    fn try_from(status: &str) -> Result<Self, Self::Error> {
        match status {
            "NEW" => Ok(LifecycleStatus::New),
            "COMPLETE" => Ok(LifecycleStatus::Complete),
            _ => Err(LifecycleStoreError::Internal(InternalError::with_message(
                format!("Unknown status {}", status),
            ))),
        }
    }
}

impl From<&LifecycleStatus> for String {
    fn from(status: &LifecycleStatus) -> Self {
        match *status {
            LifecycleStatus::New => "NEW".into(),
            LifecycleStatus::Complete => "COMPLETE".into(),
        }
    }
}
