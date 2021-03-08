// Copyright 2018-2021 Cargill Incorporated
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

//! Contains the implementation of `Node`.

pub mod admin;

use std::thread::JoinHandle;

use splinter::admin::client::{AdminServiceClient, ReqwestAdminServiceClient};
use splinter::error::InternalError;
use splinter::rest_api::actix_web_1::RestApiShutdownHandle;
use splinter::rest_api::actix_web_3::RestApi;
use splinter::threading::lifecycle::ShutdownHandle;

pub(super) enum NodeRestApiVariant {
    ActixWeb1(RestApiShutdownHandle, JoinHandle<()>),
    ActixWeb3(RestApi),
}

/// A running instance of a Splinter node.
pub struct Node {
    pub(super) admin_subsystem: admin::AdminSubsystem,
    pub(super) rest_api_variant: NodeRestApiVariant,
    pub(super) rest_api_port: u16,
}

impl Node {
    pub fn node_id(&self) -> &str {
        self.admin_subsystem.node_id()
    }

    pub fn rest_api_port(self: &Node) -> u16 {
        self.rest_api_port
    }

    pub fn admin_service_client(self: &Node) -> Box<dyn AdminServiceClient> {
        Box::new(ReqwestAdminServiceClient::new(
            format!("http://localhost:{}", self.rest_api_port),
            "foo".to_string(),
        ))
    }
}

impl ShutdownHandle for Node {
    fn signal_shutdown(&mut self) {
        self.admin_subsystem.signal_shutdown();
        if let NodeRestApiVariant::ActixWeb3(ref mut rest_api) = self.rest_api_variant {
            rest_api.signal_shutdown();
        }
    }

    fn wait_for_shutdown(self) -> Result<(), InternalError> {
        let mut errors = vec![];

        match self.rest_api_variant {
            NodeRestApiVariant::ActixWeb1(shutdown_handle, join_handle) => {
                shutdown_handle
                    .shutdown()
                    .map_err(|e| InternalError::from_source(Box::new(e)))?;
                if join_handle.join().is_err() {
                    errors.push(InternalError::with_message(
                        "REST API thread panicked, join() failed".to_string(),
                    ));
                }
            }
            NodeRestApiVariant::ActixWeb3(rest_api) => {
                if let Err(err) = rest_api.wait_for_shutdown() {
                    errors.push(err);
                }
            }
        }

        if let Err(err) = self.admin_subsystem.wait_for_shutdown() {
            errors.push(err);
        }

        match errors.len() {
            0 => Ok(()),
            1 => Err(errors.remove(0)),
            _ => Err(InternalError::with_message(format!(
                "Multiple errors occurred during shutdown: {}",
                errors
                    .into_iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))),
        }
    }
}
