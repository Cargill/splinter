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

use std::convert::TryFrom;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use log::error;

use rand::Rng;
use splinter::{
    error::InternalError,
    service::{FullyQualifiedServiceId, MessageSender, TimerHandler},
};

use crate::service::EchoRequest;
use crate::service::RequestStatus;
use crate::store::EchoStore;

use super::EchoMessage;

pub struct EchoTimerHandler {
    store: Box<dyn EchoStore>,
    stamp: Instant,
}

impl EchoTimerHandler {
    pub fn new(store: Box<dyn EchoStore>, stamp: Instant) -> Self {
        EchoTimerHandler { store, stamp }
    }
}

impl TimerHandler for EchoTimerHandler {
    type Message = EchoMessage;

    fn handle_timer(
        &mut self,
        sender: &dyn MessageSender<Self::Message>,
        service: FullyQualifiedServiceId,
    ) -> Result<(), InternalError> {
        // get the arguments for this service
        let service_args = self.store.get_service_arguments(&service)?;
        let error_wait_time =
            std::time::Duration::from_millis(((1.0 / service_args.error_rate()) * 1000.0) as u64);

        for peer in service_args.peers() {
            let actual_jitter = get_jitter(service_args.jitter().as_secs())?; // collect all requests sent to this peer and find average jitter?
            let message = "test";
            match self.store.get_last_sent(&service, peer)? {
                // send a message to those who haven't received a message in
                // frequency+actual_jitter
                Some(time) => {
                    let time = UNIX_EPOCH
                        .checked_add(Duration::from_secs(time as u64))
                        .ok_or_else(|| {
                            InternalError::with_message(
                                "'sent_at' timestamp could not be represented as a `SystemTime`"
                                    .to_string(),
                            )
                        })?;
                    if time_to_add_request(time, actual_jitter, service_args.frequency())? {
                        self.store.insert_request(&service, peer, message)?;
                    }
                }
                None => {
                    // the service hasn't been sent any messages yet
                    let correlation_id = self.store.insert_request(&service, peer, message)?;
                    sender.send(
                        peer,
                        EchoMessage::Request {
                            message: message.to_string(),
                            correlation_id: correlation_id as u64,
                        },
                    )?;
                }
            }
        }

        let unsent_requests = self
            .store
            .list_requests(&service, None)?
            .into_iter()
            .filter(|req| matches!(req.sent, RequestStatus::NotSent))
            .collect::<Vec<EchoRequest>>();

        for unsent in unsent_requests {
            // determine whether to emulate an error
            if Instant::now() > self.stamp + error_wait_time {
                let error_message = "test error";
                let error_at = i64::try_from(
                    SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .map_err(|err| InternalError::from_source(Box::new(err)))?
                        .as_secs(),
                )
                .map_err(|err| InternalError::from_source(Box::new(err)))?;

                self.store.insert_request_error(
                    &unsent.sender_service_id,
                    error_message,
                    error_at,
                )?;
                self.stamp = Instant::now();

                error!("Echo service error, message not sent");
            } else {
                let sent_at = SystemTime::now();

                sender.send(
                    &unsent.receiver_service_id,
                    EchoMessage::Request {
                        message: unsent.message,
                        correlation_id: unsent.correlation_id as u64,
                    },
                )?;

                let sent_at = i64::try_from(
                    sent_at
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .map_err(|err| InternalError::from_source(Box::new(err)))?
                        .as_secs(),
                )
                .map_err(|err| InternalError::from_source(Box::new(err)))?;
                // update time sent and status
                self.store.update_request_sent(
                    &service,
                    unsent.correlation_id,
                    RequestStatus::Sent,
                    Some(sent_at),
                )?;
            }
        }

        Ok(())
    }
}

fn get_jitter(jitter: u64) -> Result<i64, InternalError> {
    if jitter != 0 {
        let jitter =
            i64::try_from(jitter).map_err(|err| InternalError::from_source(Box::new(err)))?;
        Ok(rand::thread_rng().gen_range((0 - jitter)..jitter + 1))
    } else {
        Ok(0)
    }
}

fn time_to_add_request(
    time_last_sent: SystemTime,
    jitter: i64,
    frequency: &Duration,
) -> Result<bool, InternalError> {
    let elapsed = time_last_sent
        .elapsed()
        .map_err(|err| InternalError::from_source(Box::new(err)))?
        .as_secs() as i64;

    let unique_frequency = frequency.as_secs() as i64 + jitter;
    if elapsed > unique_frequency {
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_jitter() {
        let jitter = get_jitter(45).expect("failed to get jitter");
        assert!(jitter > (-45));
        assert!(jitter < (45));

        let jitter = get_jitter(0).expect("failed to get jitter");
        assert_eq!(jitter, 0);
    }

    #[test]
    fn test_time_to_add_request() {
        // time last sent was 5 seconds ago
        let time_last_sent = SystemTime::now()
            .checked_sub(Duration::from_secs(5))
            .expect("failed to get time last sent");
        // frequency + jitter is 8 so time_to_add should be false because not enough time has passed
        let time_to_add = time_to_add_request(time_last_sent, -2, &Duration::from_secs(10))
            .expect("failed to get frequency");
        assert!(!time_to_add);

        // time last sent was 5 seconds ago
        let time_last_sent = SystemTime::now()
            .checked_sub(Duration::from_secs(5))
            .expect("failed to get time last sent");
        // frequency + jitter is 2 so time_to_add should be true because enough time has passed
        let time_to_add = time_to_add_request(time_last_sent, -2, &Duration::from_secs(4))
            .expect("failed to get frequency");
        assert!(time_to_add);

        // time last sent was 5 seconds ago
        let time_last_sent = SystemTime::now()
            .checked_sub(Duration::from_secs(5))
            .expect("failed to get time last sent");
        // frequency + jitter is 0 so time_to_add should be true because enough time has passed
        let time_to_add = time_to_add_request(time_last_sent, 0, &Duration::from_secs(0))
            .expect("failed to get frequency");
        assert!(time_to_add);
    }
}
