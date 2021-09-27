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

use crate::error::InvalidStateError;
use crate::peer::{PeerAuthorizationToken, PeerTokenPair};
use crate::protos::admin::{Circuit, Circuit_AuthorizationType};

use super::{admin_service_id, PeerAuthorizationTokenReader, PeerNode};

impl PeerAuthorizationTokenReader for Circuit {
    fn list_tokens(&self, local_node: &str) -> Result<Vec<PeerTokenPair>, InvalidStateError> {
        let local_required_auth = self.get_node_token(local_node)?.ok_or_else(|| {
            InvalidStateError::with_message(format!(
                "Requested local node {} does not exist in the circuit",
                local_node,
            ))
        })?;

        self.get_members()
            .iter()
            .map(|member| match self.get_authorization_type() {
                Circuit_AuthorizationType::TRUST_AUTHORIZATION => Ok(PeerTokenPair::new(
                    PeerAuthorizationToken::from_peer_id(member.get_node_id()),
                    local_required_auth.clone(),
                )),
                Circuit_AuthorizationType::CHALLENGE_AUTHORIZATION => {
                    if !member.get_public_key().is_empty() {
                        Ok(PeerTokenPair::new(
                            PeerAuthorizationToken::from_public_key(member.get_public_key()),
                            local_required_auth.clone(),
                        ))
                    } else {
                        Err(InvalidStateError::with_message(format!(
                            "No public key set when circuit requires challenge \
                             authorization: {}",
                            self.get_circuit_id()
                        )))
                    }
                }
                _ => Err(InvalidStateError::with_message(format!(
                    "Circuit is missing authorization type: {}",
                    self.get_circuit_id()
                ))),
            })
            .collect::<Result<Vec<PeerTokenPair>, InvalidStateError>>()
    }

    fn list_nodes(&self) -> Result<Vec<PeerNode>, InvalidStateError> {
        self.get_members()
            .iter()
            .map(|member| match self.get_authorization_type() {
                Circuit_AuthorizationType::TRUST_AUTHORIZATION => Ok(PeerNode {
                    token: PeerAuthorizationToken::from_peer_id(member.get_node_id()),
                    node_id: member.get_node_id().to_string(),
                    endpoints: member.get_endpoints().to_vec(),
                    admin_service: admin_service_id(member.get_node_id()),
                }),
                Circuit_AuthorizationType::CHALLENGE_AUTHORIZATION => {
                    if !member.get_public_key().is_empty() {
                        Ok(PeerNode {
                            token: PeerAuthorizationToken::from_public_key(member.get_public_key()),
                            node_id: member.get_node_id().to_string(),
                            endpoints: member.get_endpoints().to_vec(),
                            admin_service: admin_service_id(member.get_node_id()),
                        })
                    } else {
                        Err(InvalidStateError::with_message(format!(
                            "No public key set when circuit requires challenge \
                                 authorization: {}",
                            self.get_circuit_id()
                        )))
                    }
                }
                _ => Err(InvalidStateError::with_message(format!(
                    "Circuit is missing authorization type: {}",
                    self.get_circuit_id()
                ))),
            })
            .collect::<Result<Vec<PeerNode>, InvalidStateError>>()
    }

    fn get_node_token(
        &self,
        node_id: &str,
    ) -> Result<Option<PeerAuthorizationToken>, InvalidStateError> {
        match self
            .get_members()
            .iter()
            .find(|member| member.get_node_id() == node_id)
        {
            Some(member) => match self.get_authorization_type() {
                Circuit_AuthorizationType::TRUST_AUTHORIZATION => Ok(Some(
                    PeerAuthorizationToken::from_peer_id(member.get_node_id()),
                )),
                Circuit_AuthorizationType::CHALLENGE_AUTHORIZATION => {
                    if !member.get_public_key().is_empty() {
                        Ok(Some(PeerAuthorizationToken::from_public_key(
                            member.get_public_key(),
                        )))
                    } else {
                        Err(InvalidStateError::with_message(
                            "Public key not set when required by a circuit".to_string(),
                        ))
                    }
                }
                _ => Err(InvalidStateError::with_message(format!(
                    "Circuit is missing authorization type: {}",
                    self.get_circuit_id()
                ))),
            },
            None => Ok(None),
        }
    }
}
