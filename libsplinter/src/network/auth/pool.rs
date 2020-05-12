// Copyright 2018-2020 Cargill Incorporated
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License atJJKK http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

trait FnBox {
    fn call_box(self: Box<Self>);
}

impl<F: FnOnce()> FnBox for F {
    fn call_box(self: Box<F>) {
        (*self)()
    }
}

type Job = Box<dyn FnBox + Send + 'static>;

fn new_job<F>(f: F) -> Job
where
    F: FnBox + Send + 'static,
{
    Box::new(f)
}

enum Message {
    NewJob(Job),
    Terminate,
}

#[derive(Debug)]
pub struct ThreadPoolBuildError(pub String);

impl std::error::Error for ThreadPoolBuildError {}

impl std::fmt::Display for ThreadPoolBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Default)]
pub struct ThreadPoolBuilder {
    size: Option<usize>,
    prefix: Option<String>,
}

impl ThreadPoolBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_size(mut self, size: usize) -> Self {
        self.size = Some(size);
        self
    }

    pub fn with_prefix(mut self, prefix: String) -> Self {
        self.prefix = Some(prefix);
        self
    }

    pub fn build(self) -> Result<ThreadPool, ThreadPoolBuildError> {
        let size = self
            .size
            .ok_or_else(|| ThreadPoolBuildError("Must configure thread pool size".into()))
            .and_then(|size| {
                if size == 0 {
                    Err(ThreadPoolBuildError(
                        "Must configure more than 0 threads".into(),
                    ))
                } else {
                    Ok(size)
                }
            })?;

        let prefix = self.prefix.unwrap_or_else(|| "ThreadPool-".into());

        let (sender, receiver) = mpsc::channel();

        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);
        for id in 0..size {
            workers.push(Worker::new(&prefix, id, receiver.clone())?);
        }

        Ok(ThreadPool { workers, sender })
    }
}

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Message>,
}

impl ThreadPool {
    pub fn executor(&self) -> JobExecutor {
        JobExecutor {
            sender: self.sender.clone(),
        }
    }

    pub fn shutdown_signaler(&self) -> ShutdownSignaler {
        ShutdownSignaler {
            worker_count: self.workers.len(),
            sender: self.sender.clone(),
        }
    }

    pub fn join_all(mut self) {
        for worker in &mut self.workers {
            debug!("Shutting down worker {}", worker.id);
            if let Some(thread) = worker.thread.take() {
                if let Err(_err) = thread.join() {
                    warn!("Failed to cleanly join worker thread {}", worker.id);
                }
            }
        }
    }
}

pub struct ShutdownSignaler {
    worker_count: usize,
    sender: mpsc::Sender<Message>,
}

impl ShutdownSignaler {
    pub fn shutdown(&self) {
        // Terminate all
        for _ in 0..self.worker_count {
            if let Err(_err) = self.sender.send(Message::Terminate) {
                // ignore a dropped receiver
            }
        }
    }
}

#[derive(Clone)]
pub struct JobExecutor {
    sender: mpsc::Sender<Message>,
}

impl JobExecutor {
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = new_job(f);
        if self.sender.send(Message::NewJob(job)).is_err() {
            // ignore the dropped receiver
        }
    }
}

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(
        prefix: &str,
        id: usize,
        receiver: Arc<Mutex<mpsc::Receiver<Message>>>,
    ) -> Result<Worker, ThreadPoolBuildError> {
        let thread = Some(
            thread::Builder::new()
                .name(format!("{}-{}", prefix, id))
                .spawn(move || loop {
                    let msg = {
                        let receiver = match receiver.lock() {
                            Ok(recv) => recv,
                            Err(err) => {
                                warn!(
                                    "Attempting to recover from a poisoned lock in worker {}",
                                    id
                                );
                                err.into_inner()
                            }
                        };

                        match receiver.recv() {
                            Ok(msg) => msg,
                            Err(_) => break,
                        }
                    };

                    match msg {
                        Message::NewJob(job) => {
                            trace!("Worker {} received job; executing.", id);
                            job.call_box();
                        }
                        Message::Terminate => {
                            debug!("Worker {} received terminate cmd.", id);
                            break;
                        }
                    }
                })
                .map_err(|err| {
                    ThreadPoolBuildError(format!("Unable to spawn worker thread: {}", err))
                })?,
        );

        Ok(Worker { id, thread })
    }
}
