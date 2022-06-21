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

mod igniter;
mod reactor_message;
mod reactor_shutdown_signaler;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

use crossbeam_channel::{bounded, RecvTimeoutError, Sender};
use futures::Future;
use tokio::runtime::Runtime;

use crate::events::ws::ShutdownHandle;
use crate::events::{ReactorError, WebSocketError};

pub use igniter::Igniter;
use reactor_message::ReactorMessage;
pub use reactor_shutdown_signaler::ReactorShutdownSignaler;

/// Reactor
///
/// Reactor creates a runtime environment for http related futures
/// on start up. Reactors create `Igniter` object that are used to
/// send futures to the runtime.
pub struct Reactor {
    sender: Sender<ReactorMessage>,
    thread_handle: thread::JoinHandle<()>,
    running: Arc<AtomicBool>,
}

impl Reactor {
    pub fn new() -> Self {
        let (sender, receiver) = bounded::<ReactorMessage>(10);
        let running = Arc::new(AtomicBool::new(true));
        let reactor_running = running.clone();

        let thread_builder = thread::Builder::new().name("EventReactor".into());
        let thread_handle = thread_builder
            .spawn(move || {
                let mut runtime = match Runtime::new() {
                    Ok(runtime) => runtime,
                    Err(err) => {
                        error!("Unable to create event reactor runtime: {}", err);
                        return;
                    }
                };

                let mut connections = Vec::new();
                let shutdown_errors = loop {
                    match receiver.recv_timeout(Duration::from_millis(500)) {
                        Ok(ReactorMessage::StartWs(listen)) => {
                            let (future, handle) = listen.into_shutdown_handle();
                            runtime.spawn(futures::lazy(|| future.map_err(|_| ())));
                            connections.push(handle);
                        }
                        Ok(ReactorMessage::HttpRequest(req)) => {
                            runtime.spawn(req);
                        }
                        Ok(ReactorMessage::Stop) => {
                            debug!("Shutting down event reactor");
                            reactor_running.store(false, Ordering::SeqCst);

                            break connections
                                .into_iter()
                                .map(|connection| connection.shutdown())
                                .filter_map(|res| if let Err(err) = res { Some(err) } else { None })
                                .collect::<Vec<WebSocketError>>();
                        }
                        Err(RecvTimeoutError::Timeout) => {
                            continue;
                        }
                        Err(RecvTimeoutError::Disconnected) => {
                            debug!(
                                "Event reactor sender disconnected; terminating web socket loop..."
                            );
                            break vec![];
                        }
                    }

                    let (live_connections, closed_connections): (
                        Vec<ShutdownHandle>,
                        Vec<ShutdownHandle>,
                    ) = connections.into_iter().partition(|conn| conn.running());
                    for conn in closed_connections {
                        match conn.shutdown() {
                            Ok(()) => info!("A ws connection closed"),
                            Err(err) => {
                                error!("A ws connection closed unexpectedly with error {}", err)
                            }
                        }
                    }
                    connections = live_connections;
                };

                if let Err(err) = runtime
                    .shutdown_on_idle()
                    .wait()
                    .map_err(|_| {
                        ReactorError::ReactorShutdownError(
                            "An Error occurred while shutting down Reactor".to_string(),
                        )
                    })
                    .and({
                        if shutdown_errors.is_empty() {
                            Ok(())
                        } else {
                            Err(ReactorError::ShutdownHandleErrors(shutdown_errors))
                        }
                    })
                {
                    error!("Unable to cleanly shutdown event reactor: {}", err);
                }
            })
            .expect("Unable to spawn event reactor thread");

        Self {
            sender,
            thread_handle,
            running,
        }
    }

    pub fn igniter(&self) -> Igniter {
        Igniter {
            sender: self.sender.clone(),
            reactor_running: self.running.clone(),
        }
    }

    /// Return a ReactorShutdownSignaler, used to send a shutdown signal to the reactor's
    /// background thread.
    pub fn shutdown_signaler(&self) -> ReactorShutdownSignaler {
        ReactorShutdownSignaler {
            sender: self.sender.clone(),
        }
    }

    /// Signals for shutdown and blocks the current thread until the Reactor's background thread
    /// has finished.
    #[deprecated(
        since = "0.3.12",
        note = "Please use the combination of `shutdown_signaler` and `wait_for_shutdown`"
    )]
    pub fn shutdown(self) -> Result<(), ReactorError> {
        self.shutdown_signaler().signal_shutdown()?;
        self.wait_for_shutdown()
    }

    /// Block until for the Reactor thread has shutdown.
    pub fn wait_for_shutdown(self) -> Result<(), ReactorError> {
        self.thread_handle.join().map_err(|_| {
            ReactorError::ReactorShutdownError("Failed to join Reactor thread".to_string())
        })
    }
}

impl std::default::Default for Reactor {
    fn default() -> Self {
        Self::new()
    }
}
