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

//! Provides a macro that can be used to shutdown multiple structs that implement the
//! `ShutdownHandle` trait

#[macro_export]
macro_rules! shutdown {
    ($handle:expr) => {{
        use std::sync::mpsc;
        use std::thread;
        use std::time::Duration;

        use splinter::error::InternalError;
        use splinter::threading::lifecycle::ShutdownHandle;

        let (tx, rx) = mpsc::channel();

        let timeout_tx = tx.clone();

        let timeout = match std::env::var("SPLINTER_TEST_SHUTDOWN_TIMEOUT_SECS") {
            Ok(v) => Duration::from_secs(v.parse().unwrap()),
            Err(_) => Duration::from_secs(120),
        };

        thread::spawn(move || {
            thread::sleep(timeout);
            if timeout_tx
                .send(Some(format!(
                    "shutdown did not complete after {} seconds",
                    timeout.as_secs()
                )))
                .is_err()
            {
                // ignore, the shutdown probably completed.
            }
        });

        struct UnsafeSender<T>(T);
        unsafe impl<T> Send for UnsafeSender<T> {}

        let handle = UnsafeSender($handle);

        let sh_join = thread::spawn(move || {
            let mut error = None;
            let UnsafeSender(mut sh) = handle;

            sh.signal_shutdown();

            if let Err(err) = sh.wait_for_shutdown() {
                error = Some(err.to_string());
            }

            // Send None that all handles have been successfully "waited for".
            tx.send(None).unwrap();

            error
        });

        if let Some(msg) = rx.recv().unwrap() {
            panic!("{}", msg);
        }

        let error = sh_join.join().unwrap();

        if let Some(err) = error {
            Err(InternalError::with_message(err))
        } else {
            Ok(())
        }
    }};
}
