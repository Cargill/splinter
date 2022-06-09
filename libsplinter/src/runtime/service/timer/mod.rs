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

//! This module contains a Timer and its component for waking up service handlers that have
//! pending messages

mod alarm;
mod message;
mod thread;

use std::sync::mpsc::{channel, Sender};
use std::time::Duration;

use crate::error::InternalError;
use crate::service::{MessageSenderFactory, TimerAlarm, TimerFilter, TimerHandlerFactory};
use crate::threading::{lifecycle::ShutdownHandle, pacemaker::Pacemaker};

use self::alarm::ChannelTimerAlarm;
use self::message::TimerMessage;
use self::thread::TimerThread;

pub struct Timer {
    pacemaker: Pacemaker,
    sender: Sender<TimerMessage>,
    timer_thread: TimerThread,
}

type FilterCollection = Vec<(
    Box<dyn TimerFilter + Send>,
    Box<dyn TimerHandlerFactory<Message = Vec<u8>>>,
)>;

/// Create a `Timer`
///
/// # Arguments
///
/// * `filters` - The collection of `TimerFilter`s and their associated `TimerHandlerFactory`
/// * `wake_up_interval` - How often the `TimerFilter`s will be checked for pending work
/// * `service_sender` - The `Sender` that will be used to create the `MessageSender` that will be
///    passed to the `TimerHandlers` to send messages
impl Timer {
    pub fn new(
        filters: FilterCollection,
        wake_up_interval: Duration,
        message_sender_factory: Box<dyn MessageSenderFactory<Vec<u8>>>,
    ) -> Result<Timer, InternalError> {
        let (sender, recv) = channel();
        let pacemaker = Pacemaker::builder()
            .with_interval(wake_up_interval.as_secs())
            .with_sender(sender.clone())
            .with_message_factory(|| TimerMessage::WakeUpAll)
            .start()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let timer_thread =
            TimerThread::start(filters, recv, message_sender_factory, sender.clone())?;

        Ok(Timer {
            pacemaker,
            sender,
            timer_thread,
        })
    }

    /// Get a `TimerAlarm` that can be use to prematurely wake up the `Timer`
    pub fn alarm(&self) -> Box<dyn TimerAlarm> {
        Box::new(ChannelTimerAlarm::new(self.sender.clone()))
    }
}

impl ShutdownHandle for Timer {
    fn signal_shutdown(&mut self) {
        self.pacemaker.shutdown_signaler().shutdown();
        self.timer_thread.signal_shutdown();
    }

    fn wait_for_shutdown(self) -> Result<(), InternalError> {
        debug!("Shutting down timer...");
        self.pacemaker.await_shutdown();
        self.timer_thread.wait_for_shutdown()?;
        debug!("Shutting down timer(complete)");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    use crate::service::{
        FullyQualifiedServiceId, MessageSender, Routable, ServiceId, ServiceType, TimerHandler,
    };

    struct TestTimerFilter {
        service_ids: Vec<FullyQualifiedServiceId>,
    }

    impl TimerFilter for TestTimerFilter {
        fn filter(&self) -> Result<Vec<FullyQualifiedServiceId>, InternalError> {
            Ok(self.service_ids.to_vec())
        }
    }

    const TEST_TYPES: &'static [ServiceType] = &[ServiceType::new_static("test")];

    impl Routable for TestTimerFilter {
        fn service_types(&self) -> &[ServiceType] {
            TEST_TYPES
        }
    }

    struct TestTimerHandler {}

    impl TimerHandler for TestTimerHandler {
        type Message = Vec<u8>;

        fn handle_timer(
            &mut self,
            sender: &dyn MessageSender<Self::Message>,
            _service: FullyQualifiedServiceId,
        ) -> Result<(), InternalError> {
            sender
                .send(&ServiceId::new("a000").unwrap(), b"woke-up".to_vec())
                .unwrap();
            Ok(())
        }
    }

    struct PanicTestTimerHandler {}

    impl TimerHandler for PanicTestTimerHandler {
        type Message = Vec<u8>;

        fn handle_timer(
            &mut self,
            _sender: &dyn MessageSender<Self::Message>,
            _service: FullyQualifiedServiceId,
        ) -> Result<(), InternalError> {
            panic!()
        }
    }

    #[derive(Clone)]
    struct TestTimerHandlerFactory {}

    impl TimerHandlerFactory for TestTimerHandlerFactory {
        type Message = Vec<u8>;

        fn new_handler(
            &self,
        ) -> Result<Box<dyn TimerHandler<Message = Self::Message>>, InternalError> {
            Ok(Box::new(TestTimerHandler {}))
        }

        fn clone_box(&self) -> Box<dyn TimerHandlerFactory<Message = Self::Message>> {
            Box::new(self.clone())
        }
    }

    /// either return a handle that will panic or the good handler
    #[derive(Clone)]
    struct TestPanicTimerHandlerFactory {
        return_panic: Arc<AtomicBool>,
    }

    impl TimerHandlerFactory for TestPanicTimerHandlerFactory {
        type Message = Vec<u8>;

        fn new_handler(
            &self,
        ) -> Result<Box<dyn TimerHandler<Message = Self::Message>>, InternalError> {
            if self.return_panic.load(Ordering::Relaxed) {
                Ok(Box::new(PanicTestTimerHandler {}))
            } else {
                Ok(Box::new(TestTimerHandler {}))
            }
        }

        fn clone_box(&self) -> Box<dyn TimerHandlerFactory<Message = Self::Message>> {
            Box::new(self.clone())
        }
    }

    struct TestMessageSender {
        scope: FullyQualifiedServiceId,
        tx: Sender<(FullyQualifiedServiceId, ServiceId, Vec<u8>)>,
    }

    impl MessageSender<Vec<u8>> for TestMessageSender {
        fn send(&self, to_service: &ServiceId, message: Vec<u8>) -> Result<(), InternalError> {
            self.tx
                .send((self.scope.clone(), to_service.clone(), message))
                .map_err(|_| InternalError::with_message("Receiver dropped".into()))
        }
    }

    #[derive(Clone)]
    struct TestMessageSenderFactory {
        tx: Sender<(FullyQualifiedServiceId, ServiceId, Vec<u8>)>,
    }

    impl MessageSenderFactory<Vec<u8>> for TestMessageSenderFactory {
        fn new_message_sender(
            &self,
            from_service: &FullyQualifiedServiceId,
        ) -> Result<Box<dyn MessageSender<Vec<u8>>>, InternalError> {
            Ok(Box::new(TestMessageSender {
                scope: from_service.clone(),
                tx: self.tx.clone(),
            }))
        }

        fn clone_boxed(&self) -> Box<dyn MessageSenderFactory<Vec<u8>>> {
            Box::new(self.clone())
        }
    }

    /// Test that a Timer set to 1 second interval will send multiple messages by waking up the
    /// timer handler.
    /// 1. Create Timer with 1 second wake up interval
    /// 2. Wait for two messages to be received
    /// 3. Shutdown timer
    #[test]
    fn test_timer_wake_up_all() {
        let wake_up_interval = Duration::from_secs(1);

        let (service_sender, service_recv) = channel();
        let message_sender_factory = Box::new(TestMessageSenderFactory { tx: service_sender });
        let filters: FilterCollection = vec![(
            Box::new(TestTimerFilter {
                service_ids: vec![
                    FullyQualifiedServiceId::new_from_string("abcde-12345::a000").unwrap(),
                ],
            }),
            Box::new(TestTimerHandlerFactory {}),
        )];

        let mut timer = Timer::new(filters, wake_up_interval, message_sender_factory).unwrap();

        if let Ok((_, _, msg_bytes)) = service_recv.recv_timeout(std::time::Duration::from_secs(5))
        {
            assert_eq!(msg_bytes, b"woke-up".to_vec())
        } else {
            panic!("Test timed out, timer handler did not wake up the first time")
        }

        if let Ok((_, _, msg_bytes)) = service_recv.recv_timeout(std::time::Duration::from_secs(5))
        {
            assert_eq!(msg_bytes, b"woke-up".to_vec())
        } else {
            panic!("Test timed out, timer handle did not wake up the second time")
        }

        timer.signal_shutdown();
        timer.wait_for_shutdown().unwrap();
    }

    /// Test that an Alarm can be used to wake up the handlers
    /// 1. Create a Timer with a wake up interval of 1000, this will make sure the Timer does not
    ///    trigger during this test.
    /// 2. Get an alarm from the Timer
    /// 3. Use the alarm to wake up all timers
    /// 4. Wait for one message to be received
    /// 5. Shutdown the timer
    #[test]
    fn test_timer_wake_up() {
        // set to large interval so it wont trigger
        let wake_up_interval = Duration::from_secs(1000);

        let (service_sender, service_recv) = channel();
        let message_sender_factory = Box::new(TestMessageSenderFactory { tx: service_sender });
        let filters: FilterCollection = vec![(
            Box::new(TestTimerFilter {
                service_ids: vec![
                    FullyQualifiedServiceId::new_from_string("abcde-12345::a000").unwrap(),
                ],
            }),
            Box::new(TestTimerHandlerFactory {}),
        )];

        let mut timer = Timer::new(filters, wake_up_interval, message_sender_factory).unwrap();

        let alarm = timer.alarm();

        alarm.wake_up_all().unwrap();

        if let Ok((_, _, msg_bytes)) = service_recv.recv_timeout(std::time::Duration::from_secs(5))
        {
            assert_eq!(msg_bytes, b"woke-up".to_vec())
        } else {
            panic!("Test timed out, timer handler did not wake up the first time")
        }

        timer.signal_shutdown();
        timer.wait_for_shutdown().unwrap();
    }

    // Test that an Alarm can be used to wake up the specific service type handlers
    /// 1. Create a Timer with a wake up interval of 1000, this will make sure the Timer does not
    ///    trigger during this test.
    /// 2. Get an alarm from the Timer
    /// 3. Use the alarm to wake up handlers with service type "test"
    /// 4. Wait for one message to be received
    /// 5. Shutdown the timer
    #[test]
    fn test_timer_wake_up_service_type() {
        // set to large interval so it won't trigger
        let wake_up_interval = Duration::from_secs(1000);

        let (service_sender, service_recv) = channel();
        let message_sender_factory = Box::new(TestMessageSenderFactory { tx: service_sender });
        let filters: FilterCollection = vec![(
            Box::new(TestTimerFilter {
                service_ids: vec![
                    FullyQualifiedServiceId::new_from_string("abcde-12345::a000").unwrap(),
                ],
            }),
            Box::new(TestTimerHandlerFactory {}),
        )];

        let mut timer = Timer::new(filters, wake_up_interval, message_sender_factory).unwrap();

        let alarm = timer.alarm();

        alarm
            .wake_up(ServiceType::new("test").unwrap(), None)
            .unwrap();

        if let Ok((_, _, msg_bytes)) = service_recv.recv_timeout(std::time::Duration::from_secs(5))
        {
            assert_eq!(msg_bytes, b"woke-up".to_vec())
        } else {
            panic!("Test timed out, timer handler did not wake up the first time")
        }

        timer.signal_shutdown();
        timer.wait_for_shutdown().unwrap();
    }

    // Test that an Alarm can be used to wake up the specific service type handler for a specific
    // id
    /// 1. Create a Timer with a wake up interval of 1000, this will make sure the Timer does not
    ///    trigger during this test.
    /// 2. Get an alarm from the Timer
    /// 3. Use the alarm to wake up handlers with service type "test" for a specific service id
    /// 4. Wait for one message to be received
    /// 5. Shutdown the timer
    #[test]
    fn test_timer_wake_up_service_id() {
        // set to large interval so it won't trigger
        let wake_up_interval = Duration::from_secs(1000);

        let (service_sender, service_recv) = channel();
        let message_sender_factory = Box::new(TestMessageSenderFactory { tx: service_sender });
        let filters: FilterCollection = vec![(
            Box::new(TestTimerFilter {
                service_ids: vec![
                    FullyQualifiedServiceId::new_from_string("abcde-12345::a000").unwrap(),
                ],
            }),
            Box::new(TestTimerHandlerFactory {}),
        )];

        let mut timer = Timer::new(filters, wake_up_interval, message_sender_factory).unwrap();

        let alarm = timer.alarm();

        alarm
            .wake_up(
                ServiceType::new("test").unwrap(),
                Some(FullyQualifiedServiceId::new_from_string("abcde-12345::a000").unwrap()),
            )
            .unwrap();

        if let Ok((_, _, msg_bytes)) = service_recv.recv_timeout(std::time::Duration::from_secs(5))
        {
            assert_eq!(msg_bytes, b"woke-up".to_vec())
        } else {
            panic!("Test timed out, timer handler did not wake up the first time")
        }

        timer.signal_shutdown();
        timer.wait_for_shutdown().unwrap();
    }

    // Test that an Alarm used to wake up the specific service type handler for a specific
    // id that is not returned by the filter will not trigger
    /// 1. Create a Timer with a wake up interval of 1000, this will make sure the Timer does not
    ///    trigger during this test.
    /// 2. Get an alarm from the Timer
    /// 3. Use the alarm to wake up handlers with service type "test" for a bad service id
    /// 4. Wait for two seconds to make sure no message is returned
    /// 5. Shutdown the timer
    #[test]
    fn test_timer_wake_up_bad_service_id() {
        // set to large interval so it won't trigger
        let wake_up_interval = Duration::from_secs(1000);

        let (service_sender, service_recv) = channel();
        let message_sender_factory = Box::new(TestMessageSenderFactory { tx: service_sender });
        let filters: FilterCollection = vec![(
            Box::new(TestTimerFilter {
                service_ids: vec![
                    FullyQualifiedServiceId::new_from_string("abcde-12345::a000").unwrap(),
                ],
            }),
            Box::new(TestTimerHandlerFactory {}),
        )];

        let mut timer = Timer::new(filters, wake_up_interval, message_sender_factory).unwrap();

        let alarm = timer.alarm();

        alarm
            .wake_up(
                ServiceType::new("test").unwrap(),
                Some(FullyQualifiedServiceId::new_from_string("abcde-12bad::a000").unwrap()),
            )
            .unwrap();

        if let Ok(_) = service_recv.recv_timeout(std::time::Duration::from_secs(2)) {
            panic!("Should not have received a message")
        }

        timer.signal_shutdown();
        timer.wait_for_shutdown().unwrap();
    }

    /// Verify that if the TimerFilter returns the same service id twice, only one thread will be
    /// run.
    /// 1. Create a Timer with a wake up interval of 1000, this will make sure the Timer does not
    ///    trigger during this test.
    /// 2. Get an alarm from the Timer
    /// 3. Use the alarm to wake up all handlers, resulting in the same service ID be returned
    ///    twice
    /// 4. Verify that only one message is returned
    /// 5. Shutdown the timer
    #[test]
    fn test_timer_wake_up_duplicate() {
        let wake_up_interval = Duration::from_secs(1000);

        let (service_sender, service_recv) = channel();
        let message_sender_factory = Box::new(TestMessageSenderFactory { tx: service_sender });
        let filters: FilterCollection = vec![(
            Box::new(TestTimerFilter {
                service_ids: vec![
                    FullyQualifiedServiceId::new_from_string("abcde-12345::a000").unwrap(),
                    FullyQualifiedServiceId::new_from_string("abcde-12345::a000").unwrap(),
                ],
            }),
            Box::new(TestTimerHandlerFactory {}),
        )];

        let mut timer = Timer::new(filters, wake_up_interval, message_sender_factory).unwrap();

        let alarm = timer.alarm();
        alarm.wake_up_all().unwrap();

        if let Ok((_, _, msg_bytes)) = service_recv.recv_timeout(std::time::Duration::from_secs(5))
        {
            assert_eq!(msg_bytes, b"woke-up".to_vec())
        } else {
            panic!("Test timed out, timer handler did not wake up the first time")
        }

        // verify we only receive one message
        if let Ok(_) = service_recv.recv_timeout(std::time::Duration::from_secs(2)) {
            panic!("Should not have received a message")
        }

        timer.signal_shutdown();
        timer.wait_for_shutdown().unwrap();
    }

    /// Verify that if a TimerHandler for a service panics, the Timer will be properly notified
    /// that the thread has shutdown and be able to start up a new thread for the same service in
    /// the future
    /// 1. Create a Timer with a wake up interval of 1000, this will make sure the Timer does not
    ///    trigger during this test. The TimerHandlerFactory is configured to return a panicking
    ///    TimerHandler.
    /// 2. Get an alarm from the Timer
    /// 3. Use the alarm to wake up all handlers
    //  4. The first TimerHandler for the service should panic
    /// 5. Update the TimerHandlerFactory to return a good TimerHandler
    /// 4. Verify that one message is returned
    /// 5. Shutdown the timer
    #[test]
    fn test_timer_wake_up_with_panics() {
        let wake_up_interval = Duration::from_secs(1000);

        let (service_sender, service_recv) = channel();
        let to_panic = Arc::new(AtomicBool::new(true));
        let message_sender_factory = Box::new(TestMessageSenderFactory { tx: service_sender });
        let filters: FilterCollection = vec![(
            Box::new(TestTimerFilter {
                service_ids: vec![
                    FullyQualifiedServiceId::new_from_string("abcde-12345::a000").unwrap(),
                ],
            }),
            Box::new(TestPanicTimerHandlerFactory {
                return_panic: to_panic.clone(),
            }),
        )];

        let mut timer = Timer::new(filters, wake_up_interval, message_sender_factory).unwrap();

        let alarm = timer.alarm();
        alarm.wake_up_all().unwrap();

        // wait for timer handler to panic
        // we cannot receive here and wait for timeout because timer handler Receiver will panic
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Update TimerHandlerFactory to return a good TimreHandler
        to_panic.store(false, Ordering::Relaxed);
        // rewake up service
        alarm.wake_up_all().unwrap();

        // verify a message is returned. This means the Timer was properly updated that the
        // previous thread for the service had shutdown so a new one should be started.
        if let Ok((_, _, msg_bytes)) = service_recv.recv_timeout(std::time::Duration::from_secs(5))
        {
            assert_eq!(msg_bytes, b"woke-up".to_vec())
        } else {
            panic!("Test timed out, timer handler did not wake up the first time")
        }

        // verify we only receive one message
        if let Ok(_) = service_recv.recv_timeout(std::time::Duration::from_secs(2)) {
            panic!("Should not have received a message")
        }

        timer.signal_shutdown();
        timer.wait_for_shutdown().unwrap();
    }
}
