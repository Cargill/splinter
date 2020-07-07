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

//! Type for notifications that the peer manager sends out.
//!
//! The public interface includes the enum [`PeerManagerNotification`]

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, TryRecvError};

use super::error::PeerManagerError;

/// Messages that will be dispatched to all subscription handlers
#[derive(Debug, PartialEq, Clone)]
pub enum PeerManagerNotification {
    /// Notifies subscribers that a peer is connected. Includes the peer ID of the connected peer.
    Connected { peer: String },
    /// Notifies subscribers that a peer is disconnected. Include the peer ID of the disconnected
    /// peer.
    Disconnected { peer: String },
}

/// `PeerNotificationIter` is used to receive notfications from the `PeerManager`. The notifications
/// include:
/// - `PeerManagerNotification::Disconnected`: peer disconnected and reconnection is being
///   attempted
/// - `PeerManagerNotification::Connected`: connection to peer was successful
pub struct PeerNotificationIter {
    pub(super) recv: Receiver<PeerManagerNotification>,
}

impl PeerNotificationIter {
    pub fn try_next(&self) -> Result<Option<PeerManagerNotification>, PeerManagerError> {
        match self.recv.try_recv() {
            Ok(notifications) => Ok(Some(notifications)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(PeerManagerError::SendMessageError(
                "The peer manager is no longer running".into(),
            )),
        }
    }
}

impl Iterator for PeerNotificationIter {
    type Item = PeerManagerNotification;

    fn next(&mut self) -> Option<Self::Item> {
        match self.recv.recv() {
            Ok(notification) => Some(notification),
            Err(_) => {
                // This is expected if the peer manager shuts down before
                // this end
                None
            }
        }
    }
}

pub type SubscriberId = usize;
pub(super) type Subscriber =
    Box<dyn Fn(PeerManagerNotification) -> Result<(), Box<dyn std::error::Error>> + Send>;

/// Responsible for broadcasting peer manager notifications.
pub(super) struct SubscriberMap {
    subscribers: HashMap<SubscriberId, Subscriber>,
    next_id: SubscriberId,
}

impl SubscriberMap {
    pub fn new() -> Self {
        Self {
            subscribers: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn broadcast(&mut self, notification: PeerManagerNotification) {
        let mut failures = vec![];
        for (id, callback) in self.subscribers.iter() {
            if let Err(err) = (*callback)(notification.clone()) {
                failures.push(*id);
                debug!("Dropping subscriber ({}): {}", id, err);
            }
        }

        for id in failures {
            self.subscribers.remove(&id);
        }
    }

    pub fn add_subscriber(&mut self, subscriber: Subscriber) -> SubscriberId {
        let subscriber_id = self.next_id;
        self.next_id += 1;
        self.subscribers.insert(subscriber_id, subscriber);

        subscriber_id
    }

    pub fn remove_subscriber(&mut self, subscriber_id: SubscriberId) {
        self.subscribers.remove(&subscriber_id);
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::sync::mpsc::channel;
    use std::thread;

    /// Tests that notifier iterator correctly exists when sender
    /// is dropped.
    ///
    /// Procedure:
    ///
    /// The test creates a channel and a notifier, then it
    /// creates a thread that sends Connected notifications to
    /// the notifier.
    ///
    /// Asserts:
    ///
    /// The notifications sent are received by the NotificationIter
    /// correctly
    ///
    /// That the total number of notifications sent equals 5
    #[test]
    fn test_peer_manager_notifications() {
        let (send, recv) = channel();

        let notifcation_iter = PeerNotificationIter { recv };

        let join_handle = thread::spawn(move || {
            for i in 0..5 {
                send.send(PeerManagerNotification::Connected {
                    peer: format!("test_peer{}", i),
                })
                .unwrap();
            }
        });

        let mut notifications_sent = 0;
        for notifcation in notifcation_iter {
            assert_eq!(
                notifcation,
                PeerManagerNotification::Connected {
                    peer: format!("test_peer{}", notifications_sent),
                }
            );
            notifications_sent += 1;
        }

        assert_eq!(notifications_sent, 5);

        join_handle.join().unwrap();
    }
}
