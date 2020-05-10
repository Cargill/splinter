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

use std::sync::mpsc::Receiver;
use std::sync::mpsc::TryRecvError;

use super::error::ConnectionManagerError;

/// Messages that will be dispatched to all subscription handlers
#[derive(Debug, PartialEq, Clone)]
pub enum ConnectionManagerNotification {
    Connected {
        endpoint: String,
        connection_id: String,
        identity: String,
    },
    FatalConnectionError {
        endpoint: String,
        error: ConnectionManagerError,
    },
    InboundConnection {
        endpoint: String,
        connection_id: String,
        identity: String,
    },
    Disconnected {
        endpoint: String,
    },
    NonFatalConnectionError {
        endpoint: String,
        attempts: u64,
    },
}

/// An iterator over ConnectionManagerNotification values
pub struct NotificationIter {
    pub(super) recv: Receiver<ConnectionManagerNotification>,
}

impl NotificationIter {
    /// Try to get the next notificaion, if it is available.
    pub fn try_next(
        &self,
    ) -> Result<Option<ConnectionManagerNotification>, ConnectionManagerError> {
        match self.recv.try_recv() {
            Ok(notifications) => Ok(Some(notifications)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(ConnectionManagerError::SendMessageError(
                "The connection manager is no longer running".into(),
            )),
        }
    }
}

impl Iterator for NotificationIter {
    type Item = ConnectionManagerNotification;

    fn next(&mut self) -> Option<Self::Item> {
        match self.recv.recv() {
            Ok(notification) => Some(notification),
            Err(_) => {
                // This is expected if the connection manager shuts down before
                // this end
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::mpsc;
    use std::thread;

    #[test]
    /// Tests that notifier iterator correctly exists when sender
    /// is dropped.
    ///
    /// Procedure:
    ///
    /// The test creates a sync channel and a notifier, then it
    /// creates a thread that send Connected notifications to
    /// the notifier.
    ///
    /// Asserts:
    ///
    /// The notifications sent are received by the NotificationIter
    /// correctly
    ///
    /// That the total number of notifications sent equals 5
    fn test_notifications_handler_iterator() {
        let (send, recv) = mpsc::channel();

        let nh = NotificationIter { recv };

        let join_handle = thread::spawn(move || {
            for _ in 0..5 {
                send.send(ConnectionManagerNotification::Connected {
                    endpoint: "tcp://localhost:3030".to_string(),
                    identity: "test".to_string(),
                    connection_id: "test_connection_id".to_string(),
                })
                .unwrap();
            }
        });

        let mut notifications_sent = 0;
        for n in nh {
            assert_eq!(
                n,
                ConnectionManagerNotification::Connected {
                    endpoint: "tcp://localhost:3030".to_string(),
                    identity: "test".to_string(),
                    connection_id: "test_connection_id".to_string(),
                }
            );
            notifications_sent += 1;
        }

        assert_eq!(notifications_sent, 5);

        join_handle.join().unwrap();
    }
}
