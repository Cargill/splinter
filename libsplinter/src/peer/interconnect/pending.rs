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

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::Receiver;
use std::time::Instant;

use protobuf::Message;

use crate::network::dispatch::DispatchMessageSender;
use crate::peer::connector::PeerLookup;
use crate::protos::network::{NetworkMessage, NetworkMessageType};
use crate::transport::matrix::{ConnectionMatrixEnvelope, ConnectionMatrixSender};

use super::PeerTokenPair;

const DEFAULT_PENDING_QUEUE_SIZE: usize = 100;
pub const DEFAULT_TIME_BETWEEN_ATTEMPTS: u64 = 10; // 10 seconds

/// Internal struct for keeping track of an pending message whose peer was not known at time of
/// receipt.
pub struct PendingIncomingMsg {
    pub envelope: ConnectionMatrixEnvelope,
    pub last_attempt: Instant,
    pub remaining_attempts: usize,
}

/// Internal struct for keeping track of an pending outgoing message whose peer connection was not
/// known at time of receipt.
pub struct PendingOutgoingMsg {
    pub recipient: PeerTokenPair,
    pub payload: Vec<u8>,
    pub last_attempt: Instant,
    pub remaining_attempts: usize,
}

/// Message for telling the pending_loop, to check if messages should be retried, a new
/// message was received whose peer is currently missing, or to shutdown.
pub enum RetryMessage {
    Retry,
    PendingIncoming(PendingIncomingMsg),
    PendingOutgoing(PendingOutgoingMsg),
    Shutdown,
}

/// This thread is in charge of retrying messages that were received but interconnect did not yet
/// have a matching peer ID for the connection ID. It is possible this peer did not exist yet due
/// to timing so it should be retried in the future. The message will be rechecked several
/// times, but if the peer is not added after a configured number of attempts the message will
/// be dropped. The number of pending queue messages is limited to a set size.
pub fn run_pending_loop<S>(
    peer_connector: &dyn PeerLookup,
    receiver: Receiver<RetryMessage>,
    dispatch_msg_sender: DispatchMessageSender<NetworkMessageType>,

    message_sender: S,
) -> Result<(), String>
where
    S: ConnectionMatrixSender + 'static,
{
    let mut connection_id_to_peer_id: HashMap<String, PeerTokenPair> = HashMap::new();
    let mut peer_id_to_connection_id: HashMap<PeerTokenPair, String> = HashMap::new();
    let mut pending_queue_incoming = VecDeque::new();
    let mut pending_queue_outgoing = VecDeque::new();
    loop {
        match receiver.recv() {
            Ok(RetryMessage::PendingIncoming(pending)) => {
                if pending_queue_incoming.len() > DEFAULT_PENDING_QUEUE_SIZE {
                    warn!(
                        "PeerInterconnect pending recv queue is to large, dropping oldest message"
                    );
                    pending_queue_incoming.pop_front();
                }
                pending_queue_incoming.push_back(pending);
                continue;
            }
            Ok(RetryMessage::PendingOutgoing(pending)) => {
                if pending_queue_outgoing.len() > DEFAULT_PENDING_QUEUE_SIZE {
                    warn!(
                        "PeerInterconnect pending send queue is to large, dropping oldest message"
                    );
                    pending_queue_outgoing.pop_front();
                }
                pending_queue_outgoing.push_back(pending);
                continue;
            }
            Ok(RetryMessage::Retry) => (),
            Ok(RetryMessage::Shutdown) => {
                info!("Received Shutdown");
                break Ok(());
            }
            Err(_) => break Err("Pending retry receiver dropped".to_string()),
        };

        let mut still_need_retry_incoming = VecDeque::new();
        for mut pending in pending_queue_incoming.into_iter() {
            if pending.last_attempt.elapsed().as_secs() < DEFAULT_TIME_BETWEEN_ATTEMPTS {
                still_need_retry_incoming.push_back(pending);
                continue;
            }

            let connection_id = pending.envelope.id().to_string();
            let peer_id = if let Some(peer_id) = connection_id_to_peer_id.get(&connection_id) {
                Some(peer_id.to_owned())
            } else if let Some(peer_id) = peer_connector
                .peer_id(&connection_id)
                .map_err(|err| format!("Unable to get peer ID for {}: {}", connection_id, err))?
            {
                connection_id_to_peer_id.insert(connection_id.to_string(), peer_id.clone());
                Some(peer_id)
            } else {
                None
            };

            // If we have the peer, pass message to dispatcher, otherwise check if we should drop
            // the message
            if let Some(peer_id) = peer_id {
                let mut network_msg: NetworkMessage =
                    match Message::parse_from_bytes(pending.envelope.payload()) {
                        Ok(msg) => msg,
                        Err(err) => {
                            error!("Unable to dispatch message: {}", err);
                            continue;
                        }
                    };

                trace!(
                    "Received message from {}({}): {:?}",
                    peer_id,
                    connection_id,
                    network_msg.get_message_type()
                );
                match dispatch_msg_sender.send(
                    network_msg.get_message_type(),
                    network_msg.take_payload(),
                    peer_id.into(),
                ) {
                    Ok(()) => (),
                    Err((message_type, _, _)) => {
                        error!("Unable to dispatch message of type {:?}", message_type)
                    }
                }
            } else if pending.remaining_attempts > 0 {
                pending.remaining_attempts -= 1;
                debug!(
                    "Received message from removed or unknown peer with connection_id {},\
                         attempts left {}",
                    connection_id, pending.remaining_attempts
                );
                still_need_retry_incoming.push_back(pending);
            } else {
                error!(
                    "Received message from removed or unknown peer with connection_id {},\
                    dropping",
                    connection_id
                );
            }
        }

        pending_queue_incoming = still_need_retry_incoming;

        let mut still_need_retry_outgoing = VecDeque::new();
        for mut pending in pending_queue_outgoing.into_iter() {
            if pending.last_attempt.elapsed().as_secs() < DEFAULT_TIME_BETWEEN_ATTEMPTS {
                still_need_retry_outgoing.push_back(pending);
                continue;
            }

            // convert recipient (peer_id) to connection_id
            let connection_id = if let Some(connection_id) =
                peer_id_to_connection_id.get(&pending.recipient)
            {
                Some(connection_id.to_owned())
            } else if let Some(connection_id) = peer_connector
                .connection_id(&pending.recipient)
                .map_err(|err| {
                    format!(
                        "Unable to get connection ID for {}: {}",
                        pending.recipient, err
                    )
                })?
            {
                peer_id_to_connection_id.insert(pending.recipient.clone(), connection_id.clone());
                Some(connection_id)
            } else {
                None
            };

            // if peer exists, send message over the network
            if let Some(connection_id) = connection_id {
                // If connection is missing, check with peer manager to see if connection id has
                // changed and try to resend message. Otherwise remove cached connection_id.
                if message_sender
                    .send(connection_id.to_string(), pending.payload.to_vec())
                    .is_err()
                {
                    if let Some(new_connection_id) = peer_connector
                        .connection_id(&pending.recipient)
                        .map_err(|err| {
                            format!(
                                "Unable to get connection ID for {}: {}",
                                &pending.recipient, err
                            )
                        })?
                    {
                        // if connection_id has changed replace it and try to send again
                        if new_connection_id != connection_id {
                            peer_id_to_connection_id
                                .insert(pending.recipient.clone(), new_connection_id.clone());
                            if message_sender
                                .send(new_connection_id, pending.payload.to_vec())
                                .is_ok()
                            {
                                // if send was successfully move on to next pending message
                                continue;
                            }
                        }
                    }
                } else {
                    // if send was successfully move on to next pending message
                    continue;
                }
            }

            // Send was not successful, check to see if the pending message still has retry attempts
            // remaining
            if pending.remaining_attempts > 0 {
                pending.remaining_attempts -= 1;
                debug!(
                    "Tried to send message to removed or unknown peer with \
                    peer_id {}, attempts left {}",
                    pending.recipient, pending.remaining_attempts
                );
                still_need_retry_outgoing.push_back(pending);
            } else {
                error!(
                    "Cannot send message, unknown peer: {}, dropping",
                    pending.recipient
                );
            }
        }
        pending_queue_outgoing = still_need_retry_outgoing;
    }
}
