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

//! Provides an implementation of [ShutdownHandle] for adapting legacy shutdown API's to the new
//! trait.

use splinter::error::InternalError;
use splinter::threading::lifecycle::ShutdownHandle;

type WaitForShutdownFn = Box<dyn FnOnce() -> Result<(), InternalError>>;

/// Provides an implementation of [ShutdownHandle] for adapting legacy shutdown API's to the new
/// trait.
pub(crate) struct LegacyShutdownHandle {
    signal_fn: Option<Box<dyn FnOnce()>>,
    wait_for_shutdown_fn: WaitForShutdownFn,
}

impl LegacyShutdownHandle {
    /// Construct a new LegacyShutdownHandle.
    ///
    /// This handle is constructed using two closures, one for signaling the target component to
    /// shutdown, the other to wait until the shutdown has completed.  These two functions will be
    /// applied in their respective trait functions.
    ///
    /// Both these functions are expected to only be called once, as the many of the legacy
    /// implementations consume the struct that provide the underlying functionality, both for
    /// signaling and for waiting.
    pub fn new(signal_fn: Box<dyn FnOnce()>, wait_for_shutdown_fn: WaitForShutdownFn) -> Self {
        Self {
            signal_fn: Some(signal_fn),
            wait_for_shutdown_fn,
        }
    }
}

impl ShutdownHandle for LegacyShutdownHandle {
    fn signal_shutdown(&mut self) {
        if let Some(signal_fn) = self.signal_fn.take() {
            signal_fn();
        }
    }

    fn wait_for_shutdown(self) -> Result<(), InternalError> {
        (self.wait_for_shutdown_fn)()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::mpsc::channel;

    #[test]
    fn test_legacy_shutdown_handle() {
        let (tx, rx) = channel();

        let mut handle = LegacyShutdownHandle::new(
            Box::new(move || tx.send("test").expect("Did not send signal")),
            Box::new(move || {
                let signal = rx.recv().expect("Did not receive signal");
                assert_eq!("test", signal);
                Ok(())
            }),
        );

        handle.signal_shutdown();
        handle
            .wait_for_shutdown()
            .expect("Did not complete shutdown successfully");
    }
}
