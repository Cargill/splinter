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

//! Type for notifications that the peer manager sends out.
//!
//! The public interface includes the enum [`PeerManagerNotification`]

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{Receiver, TryRecvError};

use super::error::PeerManagerError;
use super::PeerTokenPair;

/// Messages that will be dispatched to all subscription handlers
#[derive(Debug, PartialEq, Clone)]
pub enum PeerManagerNotification {
    /// Notifies subscribers that a peer is connected. Includes the peer ID of the connected peer.
    Connected { peer: PeerTokenPair },
    /// Notifies subscribers that a peer is disconnected. Include the peer ID of the disconnected
    /// peer.
    Disconnected { peer: PeerTokenPair },
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
    queue: VecDeque<PeerManagerNotification>,
    queue_limit: usize,
    subscribers: HashMap<SubscriberId, Subscriber>,
    next_id: SubscriberId,
}

impl SubscriberMap {
    pub fn new() -> Self {
        Self::new_with_queue_limit(std::u16::MAX as usize)
    }

    /// Construct a new SubscriberMap with a limit to the size of its pending message queue.
    ///
    /// This queue is used for messages that arrive before any subscribers have been added to the
    /// map, such that no message is lost.
    pub fn new_with_queue_limit(limit: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            queue_limit: limit,
            subscribers: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn broadcast(&mut self, notification: PeerManagerNotification) {
        self.queue.push_back(notification);
        if self.queue.len() > self.queue_limit {
            // drop the oldest notification
            self.queue.pop_front();
        }

        if self.subscribers.is_empty() {
            return;
        }

        while let Some(notification) = self.queue.pop_front() {
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
    }

    pub fn add_subscriber(&mut self, subscriber: Subscriber) -> SubscriberId {
        let subscriber_id = self.next_id;
        self.next_id += 1;

        if self.subscribers.is_empty() {
            // this is the first subscriber, so move all of the messages to the callback.
            while let Some(notification) = self.queue.pop_front() {
                if let Err(err) = (*subscriber)(notification) {
                    debug!("Dropping subscriber on add ({}): {}", subscriber_id, err);
                    return subscriber_id;
                }
            }
        }

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

    use crate::peer::PeerAuthorizationToken;

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
                    peer: PeerTokenPair::new(
                        PeerAuthorizationToken::Trust {
                            peer_id: format!("test_peer{}", i),
                        },
                        PeerAuthorizationToken::Trust {
                            peer_id: "local".into(),
                        },
                    ),
                })
                .unwrap();
            }
        });

        let mut notifications_sent = 0;
        for notifcation in notifcation_iter {
            assert_eq!(
                notifcation,
                PeerManagerNotification::Connected {
                    peer: PeerTokenPair::new(
                        PeerAuthorizationToken::Trust {
                            peer_id: format!("test_peer{}", notifications_sent),
                        },
                        PeerAuthorizationToken::Trust {
                            peer_id: "local".into(),
                        },
                    ),
                }
            );
            notifications_sent += 1;
        }

        assert_eq!(notifications_sent, 5);

        join_handle.join().unwrap();
    }

    /// Tests that a subscriber map queues message until there is at least one subscriber.
    ///
    /// Procedure:
    ///
    /// 1. Create a SubscriberMap
    /// 2. Broadcast three messages.
    /// 3. Add a subscriber to the map
    /// 4. Verify that it receives the three messages
    /// 5. Add a second subscriber
    /// 6. Send a new message
    /// 7. Verify that they both receive the message, and the new subscriber only receives the
    ///    newest message.
    #[test]
    fn test_broadcast_queue() {
        let mut subscriber_map = SubscriberMap::new();

        for i in 0..3 {
            subscriber_map.broadcast(PeerManagerNotification::Connected {
                peer: PeerTokenPair::new(
                    PeerAuthorizationToken::Trust {
                        peer_id: format!("test_peer_{}", i),
                    },
                    PeerAuthorizationToken::Trust {
                        peer_id: "local".into(),
                    },
                ),
            })
        }

        let (tx, sub1) = channel();
        let _sub1_id = subscriber_map.add_subscriber(Box::new(move |notification| {
            tx.send(notification).map_err(Box::from)
        }));

        assert_eq!(
            sub1.try_recv().expect("Unable to receive value"),
            PeerManagerNotification::Connected {
                peer: PeerTokenPair::new(
                    PeerAuthorizationToken::Trust {
                        peer_id: "test_peer_0".into()
                    },
                    PeerAuthorizationToken::Trust {
                        peer_id: "local".into()
                    },
                )
            }
        );
        assert_eq!(
            sub1.try_recv().expect("Unable to receive value"),
            PeerManagerNotification::Connected {
                peer: PeerTokenPair::new(
                    PeerAuthorizationToken::Trust {
                        peer_id: "test_peer_1".into()
                    },
                    PeerAuthorizationToken::Trust {
                        peer_id: "local".into()
                    },
                )
            }
        );
        assert_eq!(
            sub1.try_recv().expect("Unable to receive value"),
            PeerManagerNotification::Connected {
                peer: PeerTokenPair::new(
                    PeerAuthorizationToken::Trust {
                        peer_id: "test_peer_2".into()
                    },
                    PeerAuthorizationToken::Trust {
                        peer_id: "local".into()
                    },
                )
            }
        );

        assert!(matches!(
            sub1.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));

        let (tx, sub2) = channel();
        let _sub2_id = subscriber_map.add_subscriber(Box::new(move |notification| {
            tx.send(notification).map_err(Box::from)
        }));

        subscriber_map.broadcast(PeerManagerNotification::Connected {
            peer: PeerTokenPair::new(
                PeerAuthorizationToken::Trust {
                    peer_id: "test_peer_3".into(),
                },
                PeerAuthorizationToken::Trust {
                    peer_id: "local".into(),
                },
            ),
        });

        assert_eq!(
            sub1.try_recv().expect("Unable to receive value"),
            PeerManagerNotification::Connected {
                peer: PeerTokenPair::new(
                    PeerAuthorizationToken::Trust {
                        peer_id: "test_peer_3".into()
                    },
                    PeerAuthorizationToken::Trust {
                        peer_id: "local".into()
                    },
                )
            }
        );
        assert_eq!(
            sub2.try_recv().expect("Unable to receive value"),
            PeerManagerNotification::Connected {
                peer: PeerTokenPair::new(
                    PeerAuthorizationToken::Trust {
                        peer_id: "test_peer_3".into()
                    },
                    PeerAuthorizationToken::Trust {
                        peer_id: "local".into()
                    },
                )
            }
        );
    }

    /// Test that the subscriber map obeys its queue limit for messages, until there is at least
    /// one subscriber.
    ///
    /// Procedure:
    ///
    /// 1. Create a SubscriberMap with a queue limit of 1
    /// 2. Broadcast three messages.
    /// 3. Add a subscriber to the map
    /// 4. Verify that it receives the third message only
    /// 5. Send two new messages
    /// 6. Verify that the subscriber still receives both messages
    #[test]
    fn test_broadcast_queue_limit() {
        let mut subscriber_map = SubscriberMap::new_with_queue_limit(1);

        for i in 0..3 {
            subscriber_map.broadcast(PeerManagerNotification::Connected {
                peer: PeerTokenPair::new(
                    PeerAuthorizationToken::Trust {
                        peer_id: format!("test_peer_{}", i),
                    },
                    PeerAuthorizationToken::Trust {
                        peer_id: "local".into(),
                    },
                ),
            })
        }

        let (tx, sub1) = channel();
        let _sub1_id = subscriber_map.add_subscriber(Box::new(move |notification| {
            tx.send(notification).map_err(Box::from)
        }));

        assert_eq!(
            sub1.try_recv().expect("Unable to receive value"),
            PeerManagerNotification::Connected {
                peer: PeerTokenPair::new(
                    PeerAuthorizationToken::Trust {
                        peer_id: "test_peer_2".into()
                    },
                    PeerAuthorizationToken::Trust {
                        peer_id: "local".into()
                    },
                )
            }
        );
        assert!(matches!(
            sub1.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));

        subscriber_map.broadcast(PeerManagerNotification::Connected {
            peer: PeerTokenPair::new(
                PeerAuthorizationToken::Trust {
                    peer_id: "test_peer_3".into(),
                },
                PeerAuthorizationToken::Trust {
                    peer_id: "local".into(),
                },
            ),
        });
        subscriber_map.broadcast(PeerManagerNotification::Connected {
            peer: PeerTokenPair::new(
                PeerAuthorizationToken::Trust {
                    peer_id: "test_peer_4".into(),
                },
                PeerAuthorizationToken::Trust {
                    peer_id: "local".into(),
                },
            ),
        });

        assert_eq!(
            sub1.try_recv().expect("Unable to receive value"),
            PeerManagerNotification::Connected {
                peer: PeerTokenPair::new(
                    PeerAuthorizationToken::Trust {
                        peer_id: "test_peer_3".into()
                    },
                    PeerAuthorizationToken::Trust {
                        peer_id: "local".into()
                    },
                )
            }
        );
        assert_eq!(
            sub1.try_recv().expect("Unable to receive value"),
            PeerManagerNotification::Connected {
                peer: PeerTokenPair::new(
                    PeerAuthorizationToken::Trust {
                        peer_id: "test_peer_4".into()
                    },
                    PeerAuthorizationToken::Trust {
                        peer_id: "local".into()
                    },
                )
            }
        );
        assert!(matches!(
            sub1.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
    }
}
