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

//! Traits and implementations for low-level message-based network communication.
//!
//! The `splinter::transport` module defines traits for low-level networking required to pass
//! messages between two endpoints. The primary traits defined here are [`Connection`],
//! [`Listener`], and [`Transport`].
//!
//! Messages are passed between the endpoints via a [`Connection`]. The sending side calls
//! [`Connection::send`] and the receiving side calls [`Connection::recv`].  Communication across
//! a transport is bi-directional, and both sides of the [`Connection`] are a sender and receiver.
//!
//! A connection can be created in two ways. The first is by initiating the connection by calling
//! [`Connection::connect`]. The second is by listening on a local endpoint for new connections
//! initiated by others; this is accomplished with a [`Listener`] and specifically calling
//! [`Listener::accept`].
//!
//! [`Connection`]: trait.Connection.html
//! [`Connection::connect`]: trait.Connection.html#tymethod.connect
//! [`Connection::recv`]: trait.Connection.html#tymethod.recv
//! [`Connection::send`]: trait.Connection.html#tymethod.send
//! [`Listener`]: trait.Listener.html
//! [`Listener::accept`]: trait.Listener.html#tymethod.accept
//! [`Transport`]: trait.Transport.html

mod error;
pub mod inproc;
pub mod multi;
#[deprecated(since = "0.3.14", note = "please use splinter::transport::socket")]
pub mod raw;
pub mod socket;
#[deprecated(since = "0.3.14", note = "please use splinter::transport::socket")]
pub mod tls;
#[cfg(feature = "ws-transport")]
pub mod ws;
#[cfg(feature = "zmq-transport")]
pub mod zmq;

use mio::Evented;

pub use error::{AcceptError, ConnectError, DisconnectError, ListenError, RecvError, SendError};

/// A bi-directional connection between two nodes
pub trait Connection: Send {
    /// Attempt to send a message consisting of bytes across the connection.
    fn send(&mut self, message: &[u8]) -> Result<(), SendError>;

    /// Attempt to receive a message consisting of bytes from the connection.
    fn recv(&mut self) -> Result<Vec<u8>, RecvError>;

    /// Return the remote endpoint address for this connection.
    ///
    /// For TCP-based connection types, this will contain the remote peer
    /// socket address.
    fn remote_endpoint(&self) -> String;

    /// Return the local endpoint address for this connection.
    ///
    /// For TCP-based connection types, this will contain the local
    /// socket address.
    fn local_endpoint(&self) -> String;

    /// Shut down the connection.
    ///
    /// After the connection has been disconnected, messages cannot be sent
    /// or received.
    fn disconnect(&mut self) -> Result<(), DisconnectError>;

    /// Returns a `mio::event::Evented` for this connection which can be used for polling.
    fn evented(&self) -> &dyn Evented;
}

pub trait Listener: Send {
    fn accept(&mut self) -> Result<Box<dyn Connection>, AcceptError>;
    fn endpoint(&self) -> String;
}

pub trait Incoming {
    fn incoming<'a>(
        &'a mut self,
    ) -> Box<dyn Iterator<Item = Result<Box<dyn Connection>, AcceptError>> + 'a>;
}

impl Incoming for dyn Listener {
    fn incoming<'a>(
        &'a mut self,
    ) -> Box<dyn Iterator<Item = Result<Box<dyn Connection>, AcceptError>> + 'a> {
        Box::new(IncomingIter::new(self))
    }
}

/// Factory-pattern based type for creating connections
pub trait Transport: Send {
    /// Indicates whether or not a given address can be used to create a connection or listener.
    fn accepts(&self, address: &str) -> bool;
    fn connect(&mut self, endpoint: &str) -> Result<Box<dyn Connection>, ConnectError>;
    fn listen(&mut self, bind: &str) -> Result<Box<dyn Listener>, ListenError>;
}

// Helper struct for extending Listener to Incoming

struct IncomingIter<'a> {
    listener: &'a mut dyn Listener,
}

impl<'a> IncomingIter<'a> {
    pub fn new(listener: &'a mut dyn Listener) -> Self {
        IncomingIter { listener }
    }
}

impl<'a> Iterator for IncomingIter<'a> {
    type Item = Result<Box<dyn Connection>, AcceptError>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.listener.accept())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::fmt::Debug;

    use std::collections::HashMap;
    use std::sync::mpsc::channel;
    use std::thread;
    use std::time::{Duration, Instant};

    use mio::{Events, Poll, PollOpt, Ready, Token};

    fn assert_ok<T, E: Debug>(result: Result<T, E>) -> T {
        match result {
            Ok(ok) => ok,
            Err(err) => panic!("Expected Ok(...), got Err({:?})", err),
        }
    }

    macro_rules! block {
        ($op:expr, $err:ident) => {{
            let start = Instant::now();
            let duration = Duration::from_millis(60000); // 60 seconds
            loop {
                assert!(start.elapsed() < duration, "blocked for too long");
                match $op {
                    Err($err::WouldBlock) => {
                        thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                    Err(err) => break Err(err),
                    Ok(ok) => break Ok(ok),
                }
            }
        }};
    }

    pub fn test_transport<T: Transport + Send + 'static>(mut transport: T, bind: &str) {
        let mut listener = assert_ok(transport.listen(bind));
        let endpoint = listener.endpoint();

        let handle = thread::spawn(move || {
            let mut client = assert_ok(transport.connect(&endpoint));
            assert_eq!(client.remote_endpoint(), endpoint);

            assert_ok(block!(client.send(&[0, 1, 2]), SendError));
            assert_eq!(vec![3, 4, 5], assert_ok(block!(client.recv(), RecvError)));
        });

        let mut server = assert_ok(listener.incoming().next().unwrap());

        assert_eq!(vec![0, 1, 2], assert_ok(block!(server.recv(), RecvError)));

        assert_ok(block!(server.send(&[3, 4, 5]), SendError));

        handle.join().unwrap();
    }

    /// Tests that we can create connections which exhibit normal polling behavior.
    ///
    /// We expect connections to initially be writable, and we expect them to be readable once the
    /// other side has sent some data. This test confirms that we properly see both readable and
    /// writable states on each connection.
    ///
    /// Additionally, this test does send messages in both directions and confirms that the
    /// messages are recieved on the other end.
    ///
    /// The process used is essentially:
    ///
    /// 1. Create a listener.
    /// 2. Create a "connector" thread. The original test thread is now the "listener" thread.
    /// 3. In the connector thread, create some connections, setup polling, and then notify the
    ///    listener thread.
    /// 4. In the listener thread, send some data across each connection, then notify the connector
    ///    thread.
    /// 5. In the connector thread, loop around poll collecting ready events and determining if we
    ///    have seen both read and write for each connection. If we have, exit the loop.
    /// 6. In the connector thread, receive the message on each connection and then send a message.
    /// 7. In the listener thread, receive the messages.
    /// 8. Join the connector thread.
    pub fn test_poll<T: Transport + Send + 'static>(mut transport: T, bind: &str) {
        // The number of connections to create during the test. The higher the number, the more
        // likely we would be to find issues which only occur occassionally. However, the higher
        // the number, the more likely we are to cause false failures because of system-level
        // concerns such as running out of file descriptors.
        const CONNECTIONS: usize = 16;

        // The timeout duration when calling poll. This duration is arbitrary but the primary
        // objectives are: a) bound poll so it will not hang the test indefinitely; b) allow the
        // main loop to exit not long after TOTAL_DURATION.
        const POLL_DURATION: u64 = 3000; // 3 seconds

        // The timeout for the entire test. The actual maximum duration of the test will be between
        // TOTAL_DURATION and TOTAL_DURATION + POLL_DURATION, depending on timing. This value is
        // set to the highest reasonable value as to not cause false failures because we did not
        // allow enough time.
        const TOTAL_DURATION: u64 = 60000; // 60 seconds

        // Bind to a port with listen() and retrieve the actual endpoint. Determining the endpoint
        // in this manner allows for ports to be system-assigned, in the case that the bind URL was
        // something like 127.0.0.1:0.
        let mut listener = transport.listen(bind).unwrap();
        let endpoint = listener.endpoint();

        // Create two sets of channels. The first is for sending message to our connector (client)
        // thread (the thread we spawn). The second is for sending messages to our listening
        // (server) thread (which is the primary thread in the test).
        let (to_listener_tx, to_listener_rx) = channel();
        let (to_connector_tx, to_connector_rx) = channel();

        // Create the connector thread.
        let handle = thread::spawn(move || {
            // Attempt to setup all the connections, and create an associated mio Token. The token
            // is used later to correlate polling events with the connection.
            let mut connections = Vec::with_capacity(CONNECTIONS);
            for i in 0..CONNECTIONS {
                connections.push((assert_ok(transport.connect(&endpoint)), Token(i)));
            }

            // Register all connections with Poller. Since we want to monitor for both read and
            // write, we request Ready::readable() and Ready::writable events. Token is used to
            // correleate to the connection.
            let poll = Poll::new().unwrap();
            for (conn, token) in &connections {
                poll.register(
                    conn.evented(),
                    *token,
                    Ready::readable() | Ready::writable(),
                    PollOpt::level(),
                )
                .unwrap();
            }

            // Notify the listener thread that this thread has finished setting up polling.
            to_listener_tx.send(()).unwrap();

            // Block waiting for the listener thread to send a message on each connection.
            to_connector_rx.recv().unwrap();

            // Keep a map which has Tokens for keys and Ready for values; we use this to determine
            // which Ready states we have seen.
            let mut readiness_map = HashMap::new();

            // The number of connections for which we have seen a readable event.
            let mut readable_count = 0;

            // The number of connections for which we have seen a writable event.
            let mut writable_count = 0;

            // If we have timed out due to TOTAL_DURATION, this flag gets set and we break out of
            // the loop.
            let mut failure = false;

            // The structure filled in by poll; the capacity is the maximum number of events poll
            // can return at once. We set this high, though if we overflow it, it should not really
            // matter since we will call poll again quickly as we go through the loop.
            let mut events = Events::with_capacity(CONNECTIONS * 8);

            // Timing setup.
            let start = Instant::now();
            let poll_duration = Duration::from_millis(POLL_DURATION);
            let total_duration = Duration::from_millis(TOTAL_DURATION);

            // Loop until the test succeeds or times out.
            loop {
                // If we have taken too long, then the test has failed; break out of the loop.
                if start.elapsed() >= total_duration {
                    failure = true;
                    break;
                }

                // Poll. This will block until there are events, up to POLL_DURATION. When working,
                // this will return almost immediately with readable and writable events for each
                // connection. If we do not get them all the first attempt, we should on the
                // subsequent poll attempts.
                poll.poll(&mut events, Some(poll_duration)).unwrap();

                // Process the events by merging the readiness state into the state maintained
                // within the readiness map.
                for (_conn, token) in &connections {
                    events
                        .iter()
                        .filter(|event| event.token() == *token)
                        .map(|event| event.readiness())
                        .for_each(|readiness| {
                            *readiness_map.entry(token).or_insert(readiness) |= readiness;
                        });
                }

                // Calculate both readable_count and writable_count, which are the number of
                // connections readable/writable.
                readable_count = readiness_map
                    .values()
                    .filter(|value| value.is_readable())
                    .count();
                writable_count = readiness_map
                    .values()
                    .filter(|value| value.is_writable())
                    .count();

                // We expect to see each connection become both readable and writable. If that
                // happens, the test is successful, break out of the loop.
                if readable_count >= CONNECTIONS && writable_count >= CONNECTIONS {
                    break;
                }
            }

            // For each connection, make sure we can receive the message sent, then send
            // a response. Sending a response here will unblock the listener thread, so it is
            // important we do this even in the error case, as the test will hang ohterwise.
            for (mut conn, _token) in connections {
                assert_eq!(b"hello".to_vec(), block!(conn.recv(), RecvError).unwrap());
                assert_ok(conn.send(b"world"));
            }

            // If we timed out, assert in a way that leaves a breadcrumb as to the state of the
            // connections.
            if failure {
                assert_eq!((CONNECTIONS, CONNECTIONS), (readable_count, writable_count));
            }
        });

        // The code below is part of the listener thread.

        // Accept all the connections that the connector thread has initiated.
        let mut connections = Vec::with_capacity(CONNECTIONS);
        for _ in 0..CONNECTIONS {
            connections.push(listener.accept().unwrap());
        }

        // Block until the connector thread tells this thread that polling has been setup. This is
        // necessary to ensure that we do not lose any readable events.
        to_listener_rx.recv().unwrap();

        // Send a message on all connections. This should make the poller generate a readable event
        // for all connections.
        for conn in &mut connections {
            block!(conn.send(b"hello"), SendError).unwrap();
        }

        // Signal the connector thread that this thread has finished sending data on all
        // connections.
        to_connector_tx.send(()).unwrap();

        // For each connection, make sure we received the message sent from the connector thread.
        for mut conn in connections {
            assert_eq!(b"world".to_vec(), block!(conn.recv(), RecvError).unwrap());
        }

        // Join the connector thread.
        handle.join().unwrap();
    }
}
