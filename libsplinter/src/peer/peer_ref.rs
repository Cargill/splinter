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

//! Data structures for building a `PeerManager` instance.
//!
//! The public interface includes the structs [`PeerRef`] and [`EndpointPeerRef`]

use crate::peer::connector::PeerRemover;

/// Used to keep track of peer references. When dropped, the `PeerRef` will send a request to the
/// `PeerManager` to remove a reference to the peer, thus removing the peer if no more references
/// exist.
#[derive(Debug, PartialEq)]
pub struct PeerRef {
    peer_id: String,
    peer_remover: PeerRemover,
}

impl PeerRef {
    /// Creates a new `PeerRef`
    pub(super) fn new(peer_id: String, peer_remover: PeerRemover) -> Self {
        PeerRef {
            peer_id,
            peer_remover,
        }
    }

    /// Returns the peer ID this reference is for
    pub fn peer_id(&self) -> &str {
        &self.peer_id
    }
}

impl Drop for PeerRef {
    fn drop(&mut self) {
        match self.peer_remover.remove_peer_ref(&self.peer_id) {
            Ok(_) => (),
            Err(err) => error!(
                "Unable to remove reference to {} on drop: {}",
                self.peer_id, err
            ),
        }
    }
}

/// Used to keep track of peer references that are created only with an endpoint. When dropped, a
/// request is sent to the `PeerManager` to remove a reference to the peer, thus removing the peer
/// if no more references exist.
#[derive(Debug, PartialEq)]
pub struct EndpointPeerRef {
    endpoint: String,
    peer_remover: PeerRemover,
}

impl EndpointPeerRef {
    /// Creates a new `EndpointPeerRef`
    pub(super) fn new(endpoint: String, peer_remover: PeerRemover) -> Self {
        EndpointPeerRef {
            endpoint,
            peer_remover,
        }
    }

    /// Returns the endpoint of the peer this reference is for
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

impl Drop for EndpointPeerRef {
    fn drop(&mut self) {
        match self
            .peer_remover
            .remove_peer_ref_by_endpoint(&self.endpoint)
        {
            Ok(_) => (),
            Err(err) => error!(
                "Unable to remove reference to peer with endpoint {} on drop: {}",
                self.endpoint, err
            ),
        }
    }
}
