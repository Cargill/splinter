// Copyright 2021 Cargill Incorporated
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

//! A logger that prints information about batches submitted to a target. The `RequestLogger`
//! reports information stored in a `HttpRequestCounter`.

use std::sync::mpsc::{channel, Sender, TryRecvError};
use std::sync::Arc;
use std::{thread, time};

use transact::error::InternalError;
use transact::workload::HttpRequestCounter;

use crate::action::time::Time;

/// Logs information about submitted batches, waiting the specified amount of time between logs.
pub struct RequestLogger {
    thread: thread::JoinHandle<()>,
    sender: Sender<ShutdownMessage>,
}

impl RequestLogger {
    /// Starts a background thread that reports information stored in an `HttpRequestCounter`,
    /// waiting the specified amount of time between reports.
    ///
    /// # Arguments
    ///
    /// * `counters` - The list of `HttpRequestCounter`s that information will be reported for
    /// * `update_time` - How often a log should be printed
    /// * `duration` - How long the `RequestLogger` should run for
    pub fn new(
        counters: Vec<Arc<HttpRequestCounter>>,
        update_time: time::Duration,
        duration: Option<Time>,
    ) -> Result<Self, InternalError> {
        let end_time = duration.map(|d| time::Instant::now() + time::Duration::from(d));
        let (sender, receiver) = channel();
        let thread = thread::Builder::new()
            .name("HttpRequestCounter-Thread".to_string())
            .spawn(move || {
                let mut last_log_time = time::Instant::now();
                loop {
                    if let Some(end_time) = end_time {
                        // Stop at the end time if one was given
                        if time::Instant::now() > end_time {
                            break;
                        }
                    }
                    match receiver.try_recv() {
                        // Recieved shutdown
                        Ok(_) => {
                            break;
                        }
                        Err(TryRecvError::Empty) => {
                            thread::sleep(update_time);
                            log(
                                &counters,
                                last_log_time.elapsed().as_secs(),
                                last_log_time.elapsed().subsec_nanos(),
                                end_time,
                            );
                            last_log_time = time::Instant::now();
                        }
                        Err(TryRecvError::Disconnected) => {
                            error!("Request logger channel has disconnected");
                            break;
                        }
                    }
                }
            })
            .map_err(|err| {
                InternalError::with_message(format!("Unable to spawn worker thread: {}", err))
            })?;
        Ok(RequestLogger { thread, sender })
    }

    /// Return a [`RequestLoggerShutdownSignaler`], used to send a shutdown signal to the request
    /// loggers's thread.
    pub fn shutdown_signaler(&self) -> RequestLoggerShutdownSignaler {
        RequestLoggerShutdownSignaler {
            sender: self.sender.clone(),
        }
    }

    /// Block until the request logger has shutdown
    pub fn wait_for_shutdown(self) -> Result<(), InternalError> {
        self.thread.join().map_err(|_| {
            InternalError::with_message("Failed to join RequestLogger thread".to_string())
        })
    }
}

/// The sender for [`RequestLogger`].
pub struct RequestLoggerShutdownSignaler {
    sender: Sender<ShutdownMessage>,
}

impl RequestLoggerShutdownSignaler {
    /// Send a shutdown message to the [`RequestLogger`].
    pub fn signal_shutdown(&self) -> Result<(), InternalError> {
        self.sender
            .send(ShutdownMessage)
            .map_err(|_| InternalError::with_message("Failed to send shutdown message".to_string()))
    }
}

fn log(
    counters: &[Arc<HttpRequestCounter>],
    seconds: u64,
    nanoseconds: u32,
    end_time: Option<time::Instant>,
) {
    let update = seconds as f64 + f64::from(nanoseconds) * 1e-9;
    for counter in counters {
        if let Some(end_time) = end_time {
            let remaining_time = if end_time > time::Instant::now() {
                end_time - time::Instant::now()
            } else {
                time::Duration::from_secs(0)
            };
            println!(
                "{}, Batches/s {:.3}, time remaining {}",
                counter,
                counter.get_batches_per_second(update),
                display_time(remaining_time),
            );
        } else {
            println!(
                "{}, Batches/s {:.3}",
                counter,
                counter.get_batches_per_second(update),
            );
        }
        counter.reset_sent_count();
        counter.reset_queue_full_count();
    }
}

/// Sent to a request logger to signal it should stop
struct ShutdownMessage;

fn display_time(time: time::Duration) -> String {
    let seconds = time.as_secs() % 60;
    let minutes = (time.as_secs() / 60) % 60;
    let hours = (time.as_secs() / 60) / 60;
    format!("{}:{}:{}", hours, minutes, seconds)
}
