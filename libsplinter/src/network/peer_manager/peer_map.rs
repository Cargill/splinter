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

use std::collections::HashMap;
use std::time::Instant;

use crate::collections::BiHashMap;

use super::error::PeerUpdateError;

#[derive(Clone, PartialEq, Debug)]
pub enum PeerStatus {
    Connected,
    Pending,
    Disconnected { retry_attempts: u64 },
}

#[derive(Clone, PartialEq, Debug)]
pub struct PeerMetadata {
    pub id: String,
    pub connection_id: String,
    pub endpoints: Vec<String>,
    pub active_endpoint: String,
    pub status: PeerStatus,
    pub last_connection_attempt: Instant,
    pub retry_frequency: u64,
}

pub struct PeerMap {
    peers: HashMap<String, PeerMetadata>,
    // Endpoint to peer id
    endpoints: HashMap<String, String>,
    initial_retry_frequency: u64,
}

/// A map of Peer IDs to peer metadata, which also maintains a redirect table for updated peer IDs.
///
/// Peer metadata includes the peer_id, the list of endpoints and the current active endpoint.
impl PeerMap {
    pub fn new(initial_retry_frequency: u64) -> Self {
        PeerMap {
            peers: HashMap::new(),
            endpoints: HashMap::new(),
            initial_retry_frequency,
        }
    }

    /// Returns the current list of peer ids.
    pub fn peer_ids(&self) -> Vec<String> {
        self.peers
            .iter()
            .map(|(_, metadata)| metadata.id.to_string())
            .collect()
    }

    /// Returns the current map of peer ids to connection_ids
    pub fn connection_ids(&self) -> BiHashMap<String, String> {
        let mut peer_to_connection_id = BiHashMap::new();
        for (peer, metadata) in self.peers.iter() {
            peer_to_connection_id.insert(peer.to_string(), metadata.connection_id.to_string());
        }

        peer_to_connection_id
    }

    /// Insert a new peer id and endpoints
    pub fn insert(
        &mut self,
        peer_id: String,
        connection_id: String,
        endpoints: Vec<String>,
        active_endpoint: String,
        status: PeerStatus,
    ) {
        let peer_metadata = PeerMetadata {
            id: peer_id.clone(),
            endpoints: endpoints.clone(),
            active_endpoint,
            status,
            connection_id,
            last_connection_attempt: Instant::now(),
            retry_frequency: self.initial_retry_frequency,
        };

        self.peers.insert(peer_id.clone(), peer_metadata);

        for endpoint in endpoints {
            self.endpoints.insert(endpoint, peer_id.clone());
        }
    }

    /// Remove a peer id and its endpoint. Returns the PeerMetdata if the peer exists.
    pub fn remove(&mut self, peer_id: &str) -> Option<PeerMetadata> {
        if let Some(peer_metadata) = self.peers.remove(&peer_id.to_string()) {
            for endpoint in peer_metadata.endpoints.iter() {
                self.endpoints.remove(endpoint);
            }

            Some(peer_metadata)
        } else {
            None
        }
    }
    /// Updates an existing peer, all fields can be updated except peer_id.
    pub fn update_peer(&mut self, peer_metadata: PeerMetadata) -> Result<(), PeerUpdateError> {
        // Only valid if the peer already exists
        if self.peers.contains_key(&peer_metadata.id) {
            for endpoint in peer_metadata.endpoints.iter() {
                self.endpoints
                    .insert(endpoint.to_string(), peer_metadata.id.clone());
            }

            self.peers
                .insert(peer_metadata.id.to_string(), peer_metadata);

            Ok(())
        } else {
            Err(PeerUpdateError(format!(
                "Unable to update peer {}, does not exist",
                peer_metadata.id
            )))
        }
    }

    /// Returns the endpoint for the given peer id
    pub fn get_peer_from_endpoint(&self, endpoint: &str) -> Option<&PeerMetadata> {
        if let Some(peer) = self.endpoints.get(endpoint) {
            self.peers.get(peer)
        } else {
            None
        }
    }

    pub fn get_by_peer_id(&self, peer_id: &str) -> Option<&PeerMetadata> {
        self.peers.get(peer_id)
    }

    pub fn get_by_connection_id(&self, connection_id: &str) -> Option<&PeerMetadata> {
        self.peers
            .values()
            .find(|meta| meta.connection_id == connection_id)
    }

    pub fn get_pending(&self) -> impl Iterator<Item = (&String, &PeerMetadata)> {
        self.peers
            .iter()
            .filter(|(_id, peer_meta)| peer_meta.status == PeerStatus::Pending)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    // Test that peer_ids() are returned correctly
    //  1. Test that an empty peer_map returns an empty vec of peer IDs
    //  2. Add two peers and test that their id are returned from peer_ids()
    //  3. Update the first peer and test the updated peer id is returned in place of the old id.
    #[test]
    fn test_get_peer_ids() {
        let mut peer_map = PeerMap::new(10);

        let peers = peer_map.peer_ids();
        assert_eq!(peers, Vec::<String>::new());

        peer_map.insert(
            "test_peer".to_string(),
            "connection_id_1".to_string(),
            vec!["test_endpoint1".to_string(), "test_endpoint2".to_string()],
            "test_endpoint2".to_string(),
            PeerStatus::Connected,
        );

        peer_map.insert(
            "next_peer".to_string(),
            "connection_id_2".to_string(),
            vec!["endpoint1".to_string(), "endpoint2".to_string()],
            "next_endpoint1".to_string(),
            PeerStatus::Connected,
        );

        let mut peers = peer_map.peer_ids();
        peers.sort();
        assert_eq!(
            peers,
            vec!["next_peer".to_string(), "test_peer".to_string()]
        );
    }

    // Test that connection_ids() returns correctly
    //  1. Test that an empty peer_map returns an empty BiHashMap
    //  2. Add two peers and test that their ids are returned from connection_ids()
    #[test]
    fn test_get_connection_ids() {
        let mut peer_map = PeerMap::new(10);

        let peers = peer_map.peer_ids();
        assert_eq!(peers, Vec::<String>::new());

        peer_map.insert(
            "test_peer".to_string(),
            "connection_id_1".to_string(),
            vec!["test_endpoint1".to_string(), "test_endpoint2".to_string()],
            "test_endpoint2".to_string(),
            PeerStatus::Connected,
        );

        peer_map.insert(
            "next_peer".to_string(),
            "connection_id_2".to_string(),
            vec!["endpoint1".to_string(), "endpoint2".to_string()],
            "next_endpoint1".to_string(),
            PeerStatus::Connected,
        );

        let peers = peer_map.connection_ids();
        assert_eq!(
            peers.get_by_key("test_peer"),
            Some(&"connection_id_1".to_string())
        );
        assert_eq!(
            peers.get_by_key("next_peer"),
            Some(&"connection_id_2".to_string())
        );
    }

    // Test that peer_metadata() return the correct PeerMetadata for the provided id
    //  1. Test that None is retured for a peer ID that does not exist
    //  2. Insert a peer
    //  3. Validate the expected PeerMetadata is returned from
    //     get_peer_from_endpoint("test_endpoint1")
    //  4. Validate same metadata is returned from get_peer_from_endpoint("test_endpoint2")
    #[test]
    fn test_get_peer_by_endpoint() {
        let mut peer_map = PeerMap::new(10);

        let peer_metadata = peer_map.get_peer_from_endpoint("bad_endpoint");
        assert_eq!(peer_metadata, None);

        peer_map.insert(
            "test_peer".to_string(),
            "connection_id".to_string(),
            vec!["test_endpoint1".to_string(), "test_endpoint2".to_string()],
            "test_endpoint2".to_string(),
            PeerStatus::Pending,
        );

        let peer_metadata = peer_map
            .get_peer_from_endpoint("test_endpoint1")
            .expect("missing expected peer_metadata");

        assert_eq!(peer_metadata.id, "test_peer".to_string());
        assert_eq!(
            peer_metadata.endpoints,
            vec!["test_endpoint1".to_string(), "test_endpoint2".to_string()]
        );
        assert_eq!(peer_metadata.active_endpoint, "test_endpoint2".to_string());
        assert_eq!(peer_metadata.status, PeerStatus::Pending);

        assert_eq!(
            Some(peer_metadata),
            peer_map.get_peer_from_endpoint("test_endpoint2")
        );
    }

    // Test that a peer can properly be added
    //  1. Insert a peer
    //  2. Check that the peer is in self.peers
    //  3. Check that the correct metadata is returned from self.peers.get()
    #[test]
    fn test_insert_peer() {
        let mut peer_map = PeerMap::new(10);

        peer_map.insert(
            "test_peer".to_string(),
            "connection_id".to_string(),
            vec!["test_endpoint1".to_string(), "test_endpoint2".to_string()],
            "test_endpoint2".to_string(),
            PeerStatus::Pending,
        );
        assert!(peer_map.peers.contains_key("test_peer"));

        let peer_metadata = peer_map
            .peers
            .get("test_peer")
            .expect("Missing peer_metadata");
        assert_eq!(peer_metadata.id, "test_peer".to_string());
        assert_eq!(
            peer_metadata.endpoints,
            vec!["test_endpoint1".to_string(), "test_endpoint2".to_string()]
        );
        assert_eq!(peer_metadata.active_endpoint, "test_endpoint2".to_string());
        assert_eq!(peer_metadata.status, PeerStatus::Pending);
    }

    // Test that a peer can be properly removed
    //  1. Test that removing a peer_id that is not in the peer map will return None
    //  2. Insert peer test_peer and verify id is in self.peers
    //  3. Verify that the correct peer_metadata is returned when removing test_peer
    #[test]
    fn test_remove_peer() {
        let mut peer_map = PeerMap::new(10);

        let peer_metdata = peer_map.remove("test_peer");

        assert_eq!(peer_metdata, None);

        peer_map.insert(
            "test_peer".to_string(),
            "connection_id".to_string(),
            vec!["test_endpoint1".to_string(), "test_endpoint2".to_string()],
            "test_endpoint2".to_string(),
            PeerStatus::Pending,
        );
        assert!(peer_map.peers.contains_key("test_peer"));

        let peer_metadata = peer_map.remove("test_peer").expect("Missing peer_metadata");
        assert!(!peer_map.peers.contains_key("test_peer"));

        assert_eq!(peer_metadata.active_endpoint, "test_endpoint2".to_string());
        assert_eq!(peer_metadata.id, "test_peer".to_string());
    }

    // Test that a peer can be updated
    //  1. Check that an error is returned if the peer does not exist
    //  2. Insert test_peer with active endpoint test_endpoint2
    //  3. Update the active enpdoint for test_peer to test_endpoint1 and set the status to
    //     disconnected
    //  4. Check that the peer's metadata now points to test_endpoint1 and the peer is disconnected
    #[test]
    fn test_get_update_active_endpoint() {
        let mut peer_map = PeerMap::new(10);
        let no_peer_metadata = PeerMetadata {
            id: "test_peer".to_string(),
            connection_id: "connection_id".to_string(),
            endpoints: vec!["test_endpoint1".to_string(), "test_endpoint2".to_string()],
            active_endpoint: "test_endpoint1".to_string(),
            status: PeerStatus::Connected,
            last_connection_attempt: Instant::now(),
            retry_frequency: 10,
        };

        if let Ok(()) = peer_map.update_peer(no_peer_metadata) {
            panic!("Should not have been able to update peer because test_peer does not exist")
        }

        peer_map.insert(
            "test_peer".to_string(),
            "connection_id".to_string(),
            vec!["test_endpoint1".to_string(), "test_endpoint2".to_string()],
            "test_endpoint2".to_string(),
            PeerStatus::Connected,
        );
        assert!(peer_map.peers.contains_key("test_peer"));

        let mut peer_metadata = peer_map
            .get_peer_from_endpoint("test_endpoint2")
            .cloned()
            .expect("Unable to retrieve peer metadata with endpoint");

        peer_metadata.active_endpoint = "test_endpoint1".to_string();
        peer_metadata.endpoints.push("new_endpoint".to_string());
        peer_metadata.status = PeerStatus::Disconnected { retry_attempts: 5 };

        peer_map
            .update_peer(peer_metadata)
            .expect("Unable to update endpoint");

        let peer_metadata = peer_map
            .peers
            .get("test_peer")
            .expect("Missing peer_metadata");

        assert_eq!(peer_metadata.id, "test_peer".to_string());
        assert_eq!(
            peer_metadata.endpoints,
            vec![
                "test_endpoint1".to_string(),
                "test_endpoint2".to_string(),
                "new_endpoint".to_string()
            ]
        );
        assert_eq!(peer_metadata.active_endpoint, "test_endpoint1".to_string());
        assert_eq!(
            peer_metadata.status,
            PeerStatus::Disconnected { retry_attempts: 5 }
        );
    }
}
