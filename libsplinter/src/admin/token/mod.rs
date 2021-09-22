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

//! A trait to make it easier to get a list of PeerAuthorizationToken from Circuits and Proposals
//!
//! This module includes implementations for store and protobuf structs

pub mod token_protobuf;
pub mod token_protocol;

use crate::error::InvalidStateError;
use crate::peer::{PeerAuthorizationToken, PeerTokenPair};

/// Struct used to correlate a `PeerAuthorizationToken` with node information
#[derive(Clone, Debug, PartialEq)]
pub struct PeerNode {
    pub token: PeerAuthorizationToken,
    pub node_id: String,
    pub endpoints: Vec<String>,
    pub admin_service: String,
}

pub trait PeerAuthorizationTokenReader {
    fn list_tokens(&self, local_node: &str) -> Result<Vec<PeerTokenPair>, InvalidStateError>;

    fn list_nodes(&self) -> Result<Vec<PeerNode>, InvalidStateError>;

    fn get_node_token(
        &self,
        node_id: &str,
    ) -> Result<Option<PeerAuthorizationToken>, InvalidStateError>;
}

pub fn admin_service_id(node_id: &str) -> String {
    format!("admin::{}", node_id)
}
