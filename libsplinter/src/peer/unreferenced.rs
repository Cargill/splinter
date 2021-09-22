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

//! Structs for keeping track of unreferenced peers

use std::collections::HashMap;
use std::time::Instant;

use super::PeerAuthorizationToken;
use super::PeerTokenPair;

/// An entry of unreferenced peers, that may have connected externally, but have not yet been
/// requested locally.
#[derive(Debug, Clone)]
pub struct UnreferencedPeer {
    pub endpoint: String,
    pub connection_id: String,
    pub local_authorization: PeerAuthorizationToken,
}

/// An entry for a peer that was only requested by endpoint.
#[derive(Debug)]
pub struct RequestedEndpoint {
    pub endpoint: String,
    pub local_authorization: PeerAuthorizationToken,
}

pub struct UnreferencedPeerState {
    pub peers: HashMap<PeerTokenPair, UnreferencedPeer>,
    // The list of endpoints that have been requested without an ID
    pub requested_endpoints: HashMap<String, RequestedEndpoint>,
    // Last time connection to the requested endpoints was tried
    pub last_connection_attempt: Instant,
    // How often to try to connect to requested endpoints
    pub retry_frequency: u64,
}

impl UnreferencedPeerState {
    pub fn new(retry_frequency: u64) -> Self {
        UnreferencedPeerState {
            peers: HashMap::default(),
            requested_endpoints: HashMap::default(),
            last_connection_attempt: Instant::now(),
            retry_frequency,
        }
    }

    pub fn get_by_connection_id(
        &self,
        connection_id: &str,
    ) -> Option<(PeerTokenPair, UnreferencedPeer)> {
        self.peers
            .iter()
            .find(|(_, peer)| peer.connection_id == connection_id)
            .map(|(id, peer)| (id.clone(), peer.clone()))
    }
}
