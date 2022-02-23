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

use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

pub fn timeout<F, T>(timeout: Duration, f: F) -> T
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (sender, receiver) = channel();
    let tsender = sender.clone();
    let _t = thread::spawn(move || {
        match sender.send(Some(f())) {
            Ok(()) => {} // everything good
            Err(_) => {} // we have been released, don't panic
        }
    });
    let _timer = thread::spawn(move || {
        std::thread::sleep(timeout.clone());
        match tsender.send(None) {
            Ok(()) => {} // oops, we timed out
            Err(_) => {} // great, the request finished already
        }
    });
    let result = receiver.recv().unwrap();

    assert!(
        !matches!(result, None),
        "Test timed out after {:?}",
        timeout
    );

    result.unwrap()
}
