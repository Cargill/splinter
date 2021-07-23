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

//! With the addition of challenge authorization, the idenity of a peer can either be a node ID
//! when using Trust authorization or a public key when using Challenge authorization. The
//! PeerAuthorizationToken will be used to idenitfy the peer on the networking level.

use std::cmp::Ordering;
use std::fmt;

#[cfg(feature = "challenge-authorization")]
use crate::hex::to_hex;
use crate::network::auth::ConnectionAuthorizationType;

/// The authorization type specific peer ID
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PeerAuthorizationToken {
    Trust {
        peer_id: String,
    },
    #[cfg(feature = "challenge-authorization")]
    Challenge {
        public_key: Vec<u8>,
    },
}

impl PeerAuthorizationToken {
    /// Get a trust token from a provided ID
    pub fn from_peer_id(peer_id: &str) -> Self {
        PeerAuthorizationToken::Trust {
            peer_id: peer_id.to_string(),
        }
    }

    #[cfg(feature = "challenge-authorization")]
    /// Get a challenge token from a provided public_key
    pub fn from_public_key(public_key: &[u8]) -> Self {
        PeerAuthorizationToken::Challenge {
            public_key: public_key.to_vec(),
        }
    }

    /// Check if the token is trust and has the provided ID
    pub fn has_peer_id(&self, peer_id: &str) -> bool {
        match self {
            PeerAuthorizationToken::Trust { peer_id: id } => peer_id == id,
            #[cfg(feature = "challenge-authorization")]
            PeerAuthorizationToken::Challenge { .. } => false,
        }
    }

    /// Get the ID if the token is trust, else None
    pub fn peer_id(&self) -> Option<&str> {
        match self {
            PeerAuthorizationToken::Trust { peer_id } => Some(&peer_id),
            #[cfg(feature = "challenge-authorization")]
            PeerAuthorizationToken::Challenge { .. } => None,
        }
    }

    #[cfg(feature = "challenge-authorization")]
    /// Get the public key if the token is challenge, else None
    pub fn public_key(&self) -> Option<&[u8]> {
        match self {
            PeerAuthorizationToken::Trust { .. } => None,
            PeerAuthorizationToken::Challenge { public_key } => Some(public_key),
        }
    }

    /// Convert the token to a string represention
    pub fn id_as_string(&self) -> String {
        match self {
            PeerAuthorizationToken::Trust { peer_id } => peer_id.to_string(),
            #[cfg(feature = "challenge-authorization")]
            PeerAuthorizationToken::Challenge { public_key } => {
                format!("public_key::{}", to_hex(public_key))
            }
        }
    }
}

impl fmt::Display for PeerAuthorizationToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PeerAuthorizationToken::Trust { peer_id } => {
                write!(f, "Trust ( peer_id: {} )", peer_id)
            }
            #[cfg(feature = "challenge-authorization")]
            PeerAuthorizationToken::Challenge { public_key } => {
                write!(f, "Challenge ( public_key: {} )", to_hex(public_key))
            }
        }
    }
}

impl From<ConnectionAuthorizationType> for PeerAuthorizationToken {
    fn from(connection_type: ConnectionAuthorizationType) -> Self {
        match connection_type {
            ConnectionAuthorizationType::Trust { identity } => {
                PeerAuthorizationToken::Trust { peer_id: identity }
            }
            #[cfg(feature = "challenge-authorization")]
            ConnectionAuthorizationType::Challenge { public_key } => {
                PeerAuthorizationToken::Challenge { public_key }
            }
        }
    }
}

impl From<PeerAuthorizationToken> for ConnectionAuthorizationType {
    fn from(peer_token: PeerAuthorizationToken) -> Self {
        match peer_token {
            PeerAuthorizationToken::Trust { peer_id } => {
                ConnectionAuthorizationType::Trust { identity: peer_id }
            }
            #[cfg(feature = "challenge-authorization")]
            PeerAuthorizationToken::Challenge { public_key } => {
                ConnectionAuthorizationType::Challenge { public_key }
            }
        }
    }
}

impl Ord for PeerAuthorizationToken {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id_as_string().cmp(&other.id_as_string())
    }
}

impl PartialOrd for PeerAuthorizationToken {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
