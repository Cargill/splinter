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

use std::thread::JoinHandle;
use std::time::Duration;

use splinter::admin::client::{AdminServiceClient, ReqwestAdminServiceClient};
use splinter::error::InternalError;
use splinter::rest_api::actix_web_1::RestApiShutdownHandle;
use splinter::rest_api::actix_web_3::RestApi;
use splinter::threading::shutdown::ShutdownHandle;

pub(super) enum NodeRestApiVariant {
    ActixWeb1(RestApiShutdownHandle, JoinHandle<()>),
    ActixWeb3(RestApi),
}

/// A running instance of a Splinter node.
pub struct Node {
    pub(super) rest_api_variant: Option<NodeRestApiVariant>,
    pub(super) rest_api_port: u16,
}

impl Node {
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
        match self.rest_api_variant.as_mut() {
            Some(NodeRestApiVariant::ActixWeb3(rest_api)) => {
                rest_api.signal_shutdown();
            }
            Some(_) | None => {}
        }
    }

    fn wait_for_shutdown(&mut self, timeout: Duration) -> Result<(), InternalError> {
        match self.rest_api_variant.take() {
            Some(NodeRestApiVariant::ActixWeb1(shutdown_handle, join_handle)) => {
                shutdown_handle
                    .shutdown()
                    .map_err(|e| InternalError::from_source(Box::new(e)))?;
                join_handle.join().map_err(|_| {
                    InternalError::with_message(
                        "REST API thread panicked, join() failed".to_string(),
                    )
                })?;
                Ok(())
            }
            Some(NodeRestApiVariant::ActixWeb3(mut rest_api)) => {
                rest_api.wait_for_shutdown(timeout)?;
                Ok(())
            }
            None => Err(InternalError::with_message(
                "wait_for_shutdown() called on already shutdown Node".to_string(),
            )),
        }
    }
}
