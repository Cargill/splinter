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

//! Data structure for keeping track of peer information

use std::collections::hash_map::Entry::Occupied;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::collections::BiHashMap;

use super::error::PeerUpdateError;
use super::{PeerAuthorizationToken, PeerTokenPair};

/// Enum for the current status of a peer
#[derive(Clone, PartialEq, Debug)]
pub enum PeerStatus {
    /// Peer is connected and is reachable
    Connected,
    /// Peer does not currently have a connection, connection is being attempted
    Pending,
    /// The peer's connection has disconnected, reconnection is being attempted
    Disconnected { retry_attempts: u64 },
}

/// The representation of a peer in the `PeerMap`
#[derive(Clone, PartialEq, Debug)]
pub struct PeerMetadata {
    /// The unique PeerAuthorizationToken ID for the peer
    pub id: PeerAuthorizationToken,
    /// The connection ID for the peer's connection
    pub connection_id: String,
    /// A list of endpoints the peer is reachable at
    pub endpoints: Vec<String>,
    /// The endpoint of the peer's current connection
    pub active_endpoint: String,
    /// The peer's current status
    pub status: PeerStatus,
    /// The last time that a peer was attempted to be connected to
    pub last_connection_attempt: Instant,
    /// How long to wait before trying to reconnect to a peer
    pub retry_frequency: u64,
    /// The required way the local node must be identified, this is required on retry
    pub required_local_auth: PeerAuthorizationToken,
}

/// A map of peer IDs to peer metadata, which also maintains a redirect table for updated peer IDs.
///
/// Peer metadata includes the peer ID, the list of endpoints, and the current active endpoint.
pub struct PeerMap {
    peers: HashMap<PeerTokenPair, PeerMetadata>,
    // Endpoint to peer id
    endpoints: HashMap<String, HashSet<PeerTokenPair>>,
    initial_retry_frequency: u64,
    #[cfg(not(feature = "challenge-authorization"))]
    local_identity: PeerAuthorizationToken,
}

impl PeerMap {
    /// Creates a new `PeerMap`
    ///
    /// # Arguments
    ///
    /// * `initial_retry_frequency` - The value to set as the retry frequency for a new peer
    /// * `local_identity` - The default required local authorization if the
    ///  `challenge-authorization` features is not enabled
    pub fn new(
        initial_retry_frequency: u64,
        #[cfg(not(feature = "challenge-authorization"))] local_identity: PeerAuthorizationToken,
    ) -> Self {
        // initialize peers metric
        gauge!("splinter.peer_manager.peers", 0.0);

        PeerMap {
            peers: HashMap::new(),
            endpoints: HashMap::new(),
            initial_retry_frequency,
            #[cfg(not(feature = "challenge-authorization"))]
            local_identity,
        }
    }

    /// Returns the current list of peer IDs
    pub fn peer_ids(&self) -> Vec<PeerAuthorizationToken> {
        self.peers
            .iter()
            .map(|(_, metadata)| metadata.id.clone())
            .collect()
    }

    /// Returns the current map of peer IDs to connection IDs
    pub fn connection_ids(&self) -> BiHashMap<PeerTokenPair, String> {
        let mut peer_to_connection_id = BiHashMap::new();
        for (peer, metadata) in self.peers.iter() {
            peer_to_connection_id.insert(peer.clone(), metadata.connection_id.to_string());
        }

        peer_to_connection_id
    }

    /// Inserts a new peer
    ///
    /// # Arguments
    ///
    /// * `peer_id` - The unique ID for the peer
    /// * `connection_id` - The connection ID for the peer's connection
    /// * `endpoint` - A list of endpoints the peer is reachable at
    /// * `active_endpoint` - The endpoint of the peer's current connection
    /// * `status` - The peer's current status
    pub fn insert(
        &mut self,
        peer_id: PeerAuthorizationToken,
        connection_id: String,
        endpoints: Vec<String>,
        active_endpoint: String,
        status: PeerStatus,
        #[cfg(feature = "challenge-authorization")] required_local_auth: PeerAuthorizationToken,
    ) {
        let peer_metadata = PeerMetadata {
            id: peer_id.clone(),
            endpoints: endpoints.clone(),
            active_endpoint,
            status,
            connection_id,
            last_connection_attempt: Instant::now(),
            retry_frequency: self.initial_retry_frequency,
            #[cfg(feature = "challenge-authorization")]
            required_local_auth: required_local_auth.clone(),
            #[cfg(not(feature = "challenge-authorization"))]
            required_local_auth: self.local_identity.clone(),
        };

        let peer_token_pair = PeerTokenPair::new(
            peer_id,
            #[cfg(feature = "challenge-authorization")]
            required_local_auth,
        );

        self.peers.insert(peer_token_pair.clone(), peer_metadata);

        for endpoint in endpoints {
            if let Some(peer_tokens) = self.endpoints.get_mut(&endpoint) {
                peer_tokens.insert(peer_token_pair.clone());
            } else {
                let mut peer_tokens = HashSet::new();
                peer_tokens.insert(peer_token_pair.clone());
                self.endpoints.insert(endpoint.clone(), peer_tokens);
            }
        }

        gauge!("splinter.peer_manager.peers", self.peers.len() as f64);
    }

    /// Removes a peer and its endpoints.
    ///
    /// # Arguments
    ///
    /// * `peer_id` - The unique ID for the peer
    ///
    /// Returns the metadata for the peer if it exists.
    pub fn remove(&mut self, peer_id: &PeerTokenPair) -> Option<PeerMetadata> {
        if let Some(peer_metadata) = self.peers.remove(peer_id) {
            for endpoint in peer_metadata.endpoints.iter() {
                if let Some(mut peer_tokens) = self.endpoints.remove(endpoint) {
                    if peer_tokens.len() > 1 {
                        peer_tokens = peer_tokens
                            .into_iter()
                            .filter(|token| token != peer_id)
                            .collect();
                        self.endpoints.insert(endpoint.clone(), peer_tokens);
                    }
                }
            }
            gauge!("splinter.peer_manager.peers", self.peers.len() as f64);
            Some(peer_metadata)
        } else {
            gauge!("splinter.peer_manager.peers", self.peers.len() as f64);
            None
        }
    }

    /// Updates an existing peer. All fields can be updated except `peer_id`.
    ///
    /// # Arguments
    ///
    /// * `peer_metadata` - The updated peer metadata for the peer
    pub fn update_peer(&mut self, peer_metadata: PeerMetadata) -> Result<(), PeerUpdateError> {
        // Only valid if the peer already exists
        let peer_token_pair = PeerTokenPair::new(
            peer_metadata.id.clone(),
            #[cfg(feature = "challenge-authorization")]
            peer_metadata.required_local_auth.clone(),
        );
        if let Occupied(mut peer_entry) = self.peers.entry(peer_token_pair.clone()) {
            for endpoint in peer_metadata.endpoints.iter() {
                if let Some(peer_tokens) = self.endpoints.get_mut(endpoint) {
                    peer_tokens.insert(peer_token_pair.clone());
                } else {
                    let mut peer_tokens = HashSet::new();
                    peer_tokens.insert(peer_token_pair.clone());
                    self.endpoints.insert(endpoint.clone(), peer_tokens);
                }
            }

            peer_entry.insert(peer_metadata);

            Ok(())
        } else {
            Err(PeerUpdateError(format!(
                "Unable to update peer {}, does not exist",
                peer_token_pair
            )))
        }
    }

    /// Returns the metadatas for the peers from the provided endpoint
    pub fn get_peer_from_endpoint(&self, endpoint: &str) -> Option<Vec<PeerMetadata>> {
        if let Some(peer_tokens) = self.endpoints.get(endpoint) {
            let mut peers = Vec::new();

            for token in peer_tokens {
                if let Some(peer_metadata) = self.peers.get(token) {
                    peers.push(peer_metadata.clone())
                }
            }

            if peers.is_empty() {
                None
            } else {
                Some(peers)
            }
        } else {
            None
        }
    }

    /// Returns the metadata for a peer from the provided peer ID
    pub fn get_by_peer_id(&self, peer_id: &PeerTokenPair) -> Option<&PeerMetadata> {
        self.peers.get(peer_id)
    }

    /// Returns the metadata for a peer from the provided connection ID
    pub fn get_by_connection_id(&self, connection_id: &str) -> Option<&PeerMetadata> {
        self.peers
            .values()
            .find(|meta| meta.connection_id == connection_id)
    }

    /// Returns the list of peers whose peer status is pending
    pub fn get_pending(&self) -> impl Iterator<Item = (&PeerTokenPair, &PeerMetadata)> {
        self.peers
            .iter()
            .filter(|(_id, peer_meta)| peer_meta.status == PeerStatus::Pending)
    }

    /// Returns true if a provided endpoint is in the `PeerMap`
    pub fn contains_endpoint(&self, endpoint: &str) -> bool {
        self.endpoints.contains_key(endpoint)
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
        let mut peer_map = PeerMap::new(
            10,
            #[cfg(not(feature = "challenge-authorization"))]
            PeerAuthorizationToken::from_peer_id("my_id"),
        );

        let peers = peer_map.peer_ids();
        assert_eq!(peers, Vec::<PeerAuthorizationToken>::new());

        peer_map.insert(
            PeerAuthorizationToken::Trust {
                peer_id: "test_peer".to_string(),
            },
            "connection_id_1".to_string(),
            vec!["test_endpoint1".to_string(), "test_endpoint2".to_string()],
            "test_endpoint2".to_string(),
            PeerStatus::Connected,
            #[cfg(feature = "challenge-authorization")]
            PeerAuthorizationToken::from_peer_id("my_id"),
        );

        peer_map.insert(
            PeerAuthorizationToken::Trust {
                peer_id: "next_peer".to_string(),
            },
            "connection_id_2".to_string(),
            vec!["endpoint1".to_string(), "endpoint2".to_string()],
            "next_endpoint1".to_string(),
            PeerStatus::Connected,
            #[cfg(feature = "challenge-authorization")]
            PeerAuthorizationToken::from_peer_id("my_id"),
        );

        let mut peers = peer_map.peer_ids();
        peers.sort();
        assert_eq!(
            peers,
            vec![
                PeerAuthorizationToken::Trust {
                    peer_id: "next_peer".to_string()
                },
                PeerAuthorizationToken::Trust {
                    peer_id: "test_peer".to_string()
                }
            ]
        );
    }

    // Test that connection_ids() returns correctly
    //  1. Test that an empty peer_map returns an empty BiHashMap
    //  2. Add two peers and test that their ids are returned from connection_ids()
    #[test]
    fn test_get_connection_ids() {
        let mut peer_map = PeerMap::new(
            10,
            #[cfg(not(feature = "challenge-authorization"))]
            PeerAuthorizationToken::from_peer_id("my_id"),
        );

        let peers = peer_map.peer_ids();
        assert_eq!(peers, Vec::<PeerAuthorizationToken>::new());

        peer_map.insert(
            PeerAuthorizationToken::Trust {
                peer_id: "test_peer".to_string(),
            },
            "connection_id_1".to_string(),
            vec!["test_endpoint1".to_string(), "test_endpoint2".to_string()],
            "test_endpoint2".to_string(),
            PeerStatus::Connected,
            #[cfg(feature = "challenge-authorization")]
            PeerAuthorizationToken::from_peer_id("my_id"),
        );

        peer_map.insert(
            PeerAuthorizationToken::Trust {
                peer_id: "next_peer".to_string(),
            },
            "connection_id_2".to_string(),
            vec!["endpoint1".to_string(), "endpoint2".to_string()],
            "next_endpoint1".to_string(),
            PeerStatus::Connected,
            #[cfg(feature = "challenge-authorization")]
            PeerAuthorizationToken::from_peer_id("my_id"),
        );

        let peers = peer_map.connection_ids();
        assert_eq!(
            peers.get_by_key(&PeerTokenPair::new(
                PeerAuthorizationToken::Trust {
                    peer_id: "test_peer".to_string(),
                },
                #[cfg(feature = "challenge-authorization")]
                PeerAuthorizationToken::Trust {
                    peer_id: "my_id".to_string(),
                },
            )),
            Some(&"connection_id_1".to_string())
        );
        assert_eq!(
            peers.get_by_key(&PeerTokenPair::new(
                PeerAuthorizationToken::Trust {
                    peer_id: "next_peer".to_string(),
                },
                #[cfg(feature = "challenge-authorization")]
                PeerAuthorizationToken::Trust {
                    peer_id: "my_id".to_string(),
                },
            )),
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
        let mut peer_map = PeerMap::new(
            10,
            #[cfg(not(feature = "challenge-authorization"))]
            PeerAuthorizationToken::from_peer_id("my_id"),
        );

        let peer_metadata = peer_map.get_peer_from_endpoint("bad_endpoint");
        assert_eq!(peer_metadata, None);

        peer_map.insert(
            PeerAuthorizationToken::Trust {
                peer_id: "test_peer".to_string(),
            },
            "connection_id".to_string(),
            vec!["test_endpoint1".to_string(), "test_endpoint2".to_string()],
            "test_endpoint2".to_string(),
            PeerStatus::Pending,
            #[cfg(feature = "challenge-authorization")]
            PeerAuthorizationToken::from_peer_id("my_id"),
        );

        let peer_metadata = peer_map
            .get_peer_from_endpoint("test_endpoint1")
            .expect("missing expected peer_metadata");

        assert!(!peer_metadata.is_empty());

        assert_eq!(
            peer_metadata[0].id,
            PeerAuthorizationToken::Trust {
                peer_id: "test_peer".to_string()
            }
        );
        assert_eq!(
            peer_metadata[0].endpoints,
            vec!["test_endpoint1".to_string(), "test_endpoint2".to_string()]
        );
        assert_eq!(
            peer_metadata[0].active_endpoint,
            "test_endpoint2".to_string()
        );
        assert_eq!(peer_metadata[0].status, PeerStatus::Pending);

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
        let mut peer_map = PeerMap::new(
            10,
            #[cfg(not(feature = "challenge-authorization"))]
            PeerAuthorizationToken::from_peer_id("my_id"),
        );

        peer_map.insert(
            PeerAuthorizationToken::Trust {
                peer_id: "test_peer".to_string(),
            },
            "connection_id".to_string(),
            vec!["test_endpoint1".to_string(), "test_endpoint2".to_string()],
            "test_endpoint2".to_string(),
            PeerStatus::Pending,
            #[cfg(feature = "challenge-authorization")]
            PeerAuthorizationToken::from_peer_id("my_id"),
        );
        assert!(peer_map.peers.contains_key(&PeerTokenPair::new(
            PeerAuthorizationToken::Trust {
                peer_id: "test_peer".to_string(),
            },
            #[cfg(feature = "challenge-authorization")]
            PeerAuthorizationToken::Trust {
                peer_id: "my_id".to_string(),
            },
        )));

        let peer_metadata = peer_map
            .peers
            .get(&PeerTokenPair::new(
                PeerAuthorizationToken::Trust {
                    peer_id: "test_peer".to_string(),
                },
                #[cfg(feature = "challenge-authorization")]
                PeerAuthorizationToken::Trust {
                    peer_id: "my_id".to_string(),
                },
            ))
            .expect("Missing peer_metadata");
        assert_eq!(
            peer_metadata.id,
            PeerAuthorizationToken::Trust {
                peer_id: "test_peer".to_string()
            }
        );
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
        let mut peer_map = PeerMap::new(
            10,
            #[cfg(not(feature = "challenge-authorization"))]
            PeerAuthorizationToken::from_peer_id("my_id"),
        );

        let peer_metdata = peer_map.remove(&PeerTokenPair::new(
            PeerAuthorizationToken::Trust {
                peer_id: "next_peer".to_string(),
            },
            #[cfg(feature = "challenge-authorization")]
            PeerAuthorizationToken::Trust {
                peer_id: "my_id".to_string(),
            },
        ));

        assert_eq!(peer_metdata, None);

        peer_map.insert(
            PeerAuthorizationToken::Trust {
                peer_id: "test_peer".to_string(),
            },
            "connection_id".to_string(),
            vec!["test_endpoint1".to_string(), "test_endpoint2".to_string()],
            "test_endpoint2".to_string(),
            PeerStatus::Pending,
            #[cfg(feature = "challenge-authorization")]
            PeerAuthorizationToken::from_peer_id("my_id"),
        );
        assert!(peer_map.peers.contains_key(&PeerTokenPair::new(
            PeerAuthorizationToken::Trust {
                peer_id: "test_peer".to_string(),
            },
            #[cfg(feature = "challenge-authorization")]
            PeerAuthorizationToken::Trust {
                peer_id: "my_id".to_string(),
            },
        )));

        let peer_metadata = peer_map
            .remove(&PeerTokenPair::new(
                PeerAuthorizationToken::Trust {
                    peer_id: "test_peer".to_string(),
                },
                #[cfg(feature = "challenge-authorization")]
                PeerAuthorizationToken::Trust {
                    peer_id: "my_id".to_string(),
                },
            ))
            .expect("Missing peer_metadata");
        assert!(!peer_map.peers.contains_key(&PeerTokenPair::new(
            PeerAuthorizationToken::Trust {
                peer_id: "test_peer".to_string(),
            },
            #[cfg(feature = "challenge-authorization")]
            PeerAuthorizationToken::Trust {
                peer_id: "my_id".to_string(),
            },
        )));

        assert_eq!(peer_metadata.active_endpoint, "test_endpoint2".to_string());
        assert_eq!(
            peer_metadata.id,
            PeerAuthorizationToken::Trust {
                peer_id: "test_peer".to_string()
            },
        );
    }

    // Test that a peer can be updated
    //  1. Check that an error is returned if the peer does not exist
    //  2. Insert test_peer with active endpoint test_endpoint2
    //  3. Update the active enpdoint for test_peer to test_endpoint1 and set the status to
    //     disconnected
    //  4. Check that the peer's metadata now points to test_endpoint1 and the peer is disconnected
    #[test]
    fn test_get_update_active_endpoint() {
        let mut peer_map = PeerMap::new(
            10,
            #[cfg(not(feature = "challenge-authorization"))]
            PeerAuthorizationToken::from_peer_id("my_id"),
        );
        let no_peer_metadata = PeerMetadata {
            id: PeerAuthorizationToken::Trust {
                peer_id: "test_peer".to_string(),
            },
            connection_id: "connection_id".to_string(),
            endpoints: vec!["test_endpoint1".to_string(), "test_endpoint2".to_string()],
            active_endpoint: "test_endpoint1".to_string(),
            status: PeerStatus::Connected,
            last_connection_attempt: Instant::now(),
            retry_frequency: 10,
            required_local_auth: PeerAuthorizationToken::from_peer_id("my_id"),
        };

        if let Ok(()) = peer_map.update_peer(no_peer_metadata) {
            panic!("Should not have been able to update peer because test_peer does not exist")
        }

        peer_map.insert(
            PeerAuthorizationToken::Trust {
                peer_id: "test_peer".to_string(),
            },
            "connection_id".to_string(),
            vec!["test_endpoint1".to_string(), "test_endpoint2".to_string()],
            "test_endpoint2".to_string(),
            PeerStatus::Connected,
            #[cfg(feature = "challenge-authorization")]
            PeerAuthorizationToken::from_peer_id("my_id"),
        );
        assert!(peer_map.peers.contains_key(&PeerTokenPair::new(
            PeerAuthorizationToken::Trust {
                peer_id: "test_peer".to_string(),
            },
            #[cfg(feature = "challenge-authorization")]
            PeerAuthorizationToken::Trust {
                peer_id: "my_id".to_string(),
            },
        )));

        let mut peer_metadata = peer_map
            .get_by_connection_id("connection_id")
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
            .get(&PeerTokenPair::new(
                PeerAuthorizationToken::Trust {
                    peer_id: "test_peer".to_string(),
                },
                #[cfg(feature = "challenge-authorization")]
                PeerAuthorizationToken::Trust {
                    peer_id: "my_id".to_string(),
                },
            ))
            .expect("Missing peer_metadata");

        assert_eq!(
            peer_metadata.id,
            PeerAuthorizationToken::Trust {
                peer_id: "test_peer".to_string(),
            }
        );
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
