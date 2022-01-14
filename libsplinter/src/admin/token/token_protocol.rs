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

use crate::admin::store::{AuthorizationType, Circuit, ProposedCircuit};
use crate::error::InvalidStateError;
use crate::peer::{PeerAuthorizationToken, PeerTokenPair};

use super::{admin_service_id, PeerAuthorizationTokenReader, PeerNode};

impl PeerAuthorizationTokenReader for ProposedCircuit {
    fn list_tokens(&self, local_node: &str) -> Result<Vec<PeerTokenPair>, InvalidStateError> {
        let local_required_auth = self.get_node_token(local_node)?.ok_or_else(|| {
            InvalidStateError::with_message(format!(
                "Requested local node {} does not exist in the circuit",
                local_node,
            ))
        })?;

        self.members()
            .iter()
            .map(|member| match self.authorization_type() {
                AuthorizationType::Trust => Ok(PeerTokenPair::new(
                    PeerAuthorizationToken::from_peer_id(member.node_id()),
                    local_required_auth.clone(),
                )),
                AuthorizationType::Challenge => {
                    if let Some(public_key) = member.public_key() {
                        Ok(PeerTokenPair::new(
                            PeerAuthorizationToken::from_public_key(public_key.as_slice()),
                            local_required_auth.clone(),
                        ))
                    } else {
                        Err(InvalidStateError::with_message(format!(
                            "No public key set when circuit requires challenge \
                             authorization: {}",
                            self.circuit_id()
                        )))
                    }
                }
            })
            .collect::<Result<Vec<PeerTokenPair>, InvalidStateError>>()
    }

    fn list_nodes(&self) -> Result<Vec<PeerNode>, InvalidStateError> {
        self.members()
            .iter()
            .map(|member| match self.authorization_type() {
                AuthorizationType::Trust => Ok(PeerNode {
                    token: PeerAuthorizationToken::from_peer_id(member.node_id()),
                    node_id: member.node_id().to_string(),
                    endpoints: member.endpoints().to_vec(),
                    admin_service: admin_service_id(member.node_id()),
                }),
                AuthorizationType::Challenge => {
                    if let Some(public_key) = member.public_key() {
                        Ok(PeerNode {
                            token: PeerAuthorizationToken::from_public_key(public_key.as_slice()),
                            node_id: member.node_id().to_string(),
                            endpoints: member.endpoints().to_vec(),
                            admin_service: admin_service_id(member.node_id()),
                        })
                    } else {
                        Err(InvalidStateError::with_message(format!(
                            "No public key set when circuit requires challenge \
                             authorization: {}",
                            self.circuit_id()
                        )))
                    }
                }
            })
            .collect::<Result<Vec<PeerNode>, InvalidStateError>>()
    }

    fn get_node_token(
        &self,
        node_id: &str,
    ) -> Result<Option<PeerAuthorizationToken>, InvalidStateError> {
        match self
            .members()
            .iter()
            .find(|member| member.node_id() == node_id)
        {
            Some(member) => match self.authorization_type() {
                AuthorizationType::Trust => {
                    Ok(Some(PeerAuthorizationToken::from_peer_id(member.node_id())))
                }
                AuthorizationType::Challenge => {
                    if let Some(public_key) = member.public_key() {
                        Ok(Some(PeerAuthorizationToken::from_public_key(
                            public_key.as_slice(),
                        )))
                    } else {
                        Err(InvalidStateError::with_message(
                            "Public key not set when required by a circuit".to_string(),
                        ))
                    }
                }
            },
            None => Ok(None),
        }
    }
}

impl PeerAuthorizationTokenReader for Circuit {
    fn list_tokens(&self, local_node: &str) -> Result<Vec<PeerTokenPair>, InvalidStateError> {
        let local_required_auth = self.get_node_token(local_node)?.ok_or_else(|| {
            InvalidStateError::with_message(format!(
                "Requested local node {} does not exist in the circuit",
                local_node,
            ))
        })?;

        self.members()
            .iter()
            .map(|member| match self.authorization_type() {
                AuthorizationType::Trust => Ok(PeerTokenPair::new(
                    PeerAuthorizationToken::from_peer_id(member.node_id()),
                    local_required_auth.clone(),
                )),
                AuthorizationType::Challenge => {
                    if let Some(public_key) = member.public_key() {
                        Ok(PeerTokenPair::new(
                            PeerAuthorizationToken::from_public_key(public_key.as_slice()),
                            local_required_auth.clone(),
                        ))
                    } else {
                        Err(InvalidStateError::with_message(format!(
                            "No public key set when circuit requires challenge \
                             authorization: {}",
                            self.circuit_id()
                        )))
                    }
                }
            })
            .collect::<Result<Vec<PeerTokenPair>, InvalidStateError>>()
    }

    fn list_nodes(&self) -> Result<Vec<PeerNode>, InvalidStateError> {
        self.members()
            .iter()
            .map(|member| match self.authorization_type() {
                AuthorizationType::Trust => Ok(PeerNode {
                    token: PeerAuthorizationToken::from_peer_id(member.node_id()),
                    node_id: member.node_id().to_string(),
                    endpoints: member.endpoints().to_vec(),
                    admin_service: admin_service_id(member.node_id()),
                }),
                AuthorizationType::Challenge => {
                    if let Some(public_key) = member.public_key() {
                        Ok(PeerNode {
                            token: PeerAuthorizationToken::from_public_key(public_key.as_slice()),
                            node_id: member.node_id().to_string(),
                            endpoints: member.endpoints().to_vec(),
                            admin_service: admin_service_id(member.node_id()),
                        })
                    } else {
                        Err(InvalidStateError::with_message(format!(
                            "No public key set when circuit requires challenge \
                             authorization: {}",
                            self.circuit_id()
                        )))
                    }
                }
            })
            .collect::<Result<Vec<PeerNode>, InvalidStateError>>()
    }

    fn get_node_token(
        &self,
        node_id: &str,
    ) -> Result<Option<PeerAuthorizationToken>, InvalidStateError> {
        match self
            .members()
            .iter()
            .find(|member| member.node_id() == node_id)
        {
            Some(member) => match self.authorization_type() {
                AuthorizationType::Trust => {
                    Ok(Some(PeerAuthorizationToken::from_peer_id(member.node_id())))
                }
                AuthorizationType::Challenge => {
                    if let Some(public_key) = member.public_key() {
                        Ok(Some(PeerAuthorizationToken::from_public_key(
                            public_key.as_slice(),
                        )))
                    } else {
                        Err(InvalidStateError::with_message(
                            "Public key not set when required by a circuit".to_string(),
                        ))
                    }
                }
            },
            None => Ok(None),
        }
    }
}
