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

        let prefix = self.prefix.unwrap_or_else(|| "ThreadPool".into());

        let (sender, receiver) = mpsc::channel();
        let (supervisor_tx, supervisor_rx) = mpsc::channel();

        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);
        for id in 0..size {
            workers.push(Worker::new(
                &prefix,
                id,
                receiver.clone(),
                supervisor_tx.clone(),
            )?);
        }
        let workers = Arc::new(Mutex::new(workers));

        let supervisor = Supervisor::new(
            prefix.clone(),
            workers.clone(),
            supervisor_rx,
            supervisor_tx.clone(),
            receiver,
        );
        let supervisor_thread = thread::Builder::new()
            .name(format!("{}-Supervisor", prefix))
            .spawn(move || supervisor.run())
            .map_err(|err| {
                ThreadPoolBuildError(format!("Unable to spawn supervisor thread: {}", err))
            })?;

        Ok(ThreadPool {
            workers,
            sender,
            supervisor_thread,
            supervisor_tx,
        })
    }
}

pub struct ThreadPool {
    workers: Arc<Mutex<Vec<Worker>>>,
    sender: mpsc::Sender<Message>,
    supervisor_thread: thread::JoinHandle<()>,
    supervisor_tx: mpsc::Sender<SupervisorSignal>,
}

impl ThreadPool {
    pub fn executor(&self) -> JobExecutor {
        JobExecutor {
            sender: self.sender.clone(),
        }
    }

    pub fn shutdown_signaler(&self) -> ShutdownSignaler {
        let worker_count = match self.workers.lock() {
            Ok(workers) => workers.len(),
            Err(err) => {
                warn!("Attempting to recover from a poisoned lock while joining",);
                err.into_inner().len()
            }
        };
        ShutdownSignaler {
            worker_count,
            sender: self.sender.clone(),
            supervisor_tx: self.supervisor_tx.clone(),
        }
    }

    pub fn join_all(self) {
        if let Err(_err) = self.supervisor_thread.join() {
            warn!("failed to cleanly join supervisor thread");
        }
        let mut workers = match self.workers.lock() {
            Ok(workers) => workers,
            Err(err) => {
                warn!("Attempting to recover from a poisoned lock while joining",);
                err.into_inner()
            }
        };
        for worker in workers.drain(..) {
            debug!("Shutting down worker {}", worker.id);
            if let Err(_err) = worker.thread.join() {
                warn!("Failed to cleanly join worker thread {}", worker.id);
            }
        }
    }
}

pub struct ShutdownSignaler {
    worker_count: usize,
    sender: mpsc::Sender<Message>,
    supervisor_tx: mpsc::Sender<SupervisorSignal>,
}

impl ShutdownSignaler {
    pub fn shutdown(&self) {
        if let Err(_err) = self.supervisor_tx.send(SupervisorSignal::Shutdown) {
            // ignore a dropped receiver
        }
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
    thread: thread::JoinHandle<()>,
}

impl Worker {
    fn new(
        prefix: &str,
        id: usize,
        receiver: Arc<Mutex<mpsc::Receiver<Message>>>,
        supervisor_tx: mpsc::Sender<SupervisorSignal>,
    ) -> Result<Worker, ThreadPoolBuildError> {
        let thread = thread::Builder::new()
            .name(format!("{}-{}", prefix, id))
            .spawn(move || {
                // we just have to hold on to this until it is dropped
                let _supervisor = PanicMonitor { id, supervisor_tx };
                loop {
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
                }
            })
            .map_err(|err| {
                ThreadPoolBuildError(format!("Unable to spawn worker thread: {}", err))
            })?;

        Ok(Worker { id, thread })
    }
}

enum SupervisorSignal {
    Restart(usize),
    Shutdown,
}

struct Supervisor {
    prefix: String,
    workers: Arc<Mutex<Vec<Worker>>>,
    supervisor_rx: mpsc::Receiver<SupervisorSignal>,
    supervisor_tx: mpsc::Sender<SupervisorSignal>,
    job_receiver: Arc<Mutex<mpsc::Receiver<Message>>>,
}

impl Supervisor {
    fn new(
        prefix: String,
        workers: Arc<Mutex<Vec<Worker>>>,
        supervisor_rx: mpsc::Receiver<SupervisorSignal>,
        supervisor_tx: mpsc::Sender<SupervisorSignal>,
        job_receiver: Arc<Mutex<mpsc::Receiver<Message>>>,
    ) -> Self {
        Self {
            prefix,
            workers,
            supervisor_rx,
            supervisor_tx,
            job_receiver,
        }
    }

    fn run(&self) {
        while let Ok(report) = self.supervisor_rx.recv() {
            match report {
                SupervisorSignal::Restart(id) => {
                    debug!("Replacing {}-{}", &self.prefix, id);

                    let mut workers = match self.workers.lock() {
                        Ok(workers) => workers,
                        Err(err) => {
                            debug!("Recovering from a poisoned lock in supervisor thread");
                            err.into_inner()
                        }
                    };

                    let old_worker = match workers.get_mut(id) {
                        Some(worker) => worker,
                        // if this is None, the workers have been drained during the shutdown join,
                        // and there's no entry for it in the list. The supervisor probably hasn't
                        // received its shutdown message yet.
                        None => continue,
                    };

                    match Worker::new(
                        &self.prefix,
                        id,
                        self.job_receiver.clone(),
                        self.supervisor_tx.clone(),
                    ) {
                        Ok(mut worker) => {
                            std::mem::swap(old_worker, &mut worker);
                            // join out the old worker:
                            if let Err(_err) = worker.thread.join() {
                                // as we know the thread panicked, we can ignore this error, which
                                // we can't log as debug anyway, due to its type.
                            } else {
                                // as we know the thread panicked, a Result::Ok variant should not
                                // be possible
                                unreachable!()
                            }
                        }
                        Err(err) => error!("Unable to restart {}-{}: {}", &self.prefix, id, err),
                    }
                }
                SupervisorSignal::Shutdown => break,
            }
        }
    }
}

struct PanicMonitor {
    id: usize,
    supervisor_tx: mpsc::Sender<SupervisorSignal>,
}

impl Drop for PanicMonitor {
    fn drop(&mut self) {
        if thread::panicking()
            && self
                .supervisor_tx
                .send(SupervisorSignal::Restart(self.id))
                .is_err()
        {
            error!(
                "Unable to notify supervisor thread of Worker {} termination due to a panic",
                self.id
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::{mpsc::channel, Arc, Barrier};
    use std::thread;

    /// Test that all threads in the pool are used by creating MAX_THREADS jobs, each that sleeps a
    /// descending number of milliseconds such that the pool will be saturated.
    #[test]
    fn test_job() -> Result<(), Box<dyn std::error::Error>> {
        let max_threads = 4;
        let thread_pool = ThreadPoolBuilder::new().with_size(max_threads).build()?;
        let (tx, rx) = channel();

        let barrier = Arc::new(Barrier::new(max_threads));
        for _ in 0..max_threads {
            let job_tx = tx.clone();
            let barrier = Arc::clone(&barrier);
            thread_pool.executor().execute(move || {
                job_tx
                    .send(
                        thread::current()
                            .name()
                            .expect("worker thread was not named")
                            .to_string(),
                    )
                    .expect("Unable to send result");

                // ensures that every worker is in use by the time the last job is run
                barrier.wait();
            });
        }

        // Drop the sender so that the receiver will close when the last job is done.
        drop(tx);

        let mut results: Vec<_> = rx.iter().collect();

        results.sort();

        assert_eq!(
            vec![
                "ThreadPool-0".to_string(),
                "ThreadPool-1".to_string(),
                "ThreadPool-2".to_string(),
                "ThreadPool-3".to_string()
            ],
            results
        );

        thread_pool.shutdown_signaler().shutdown();

        thread_pool.join_all();

        Ok(())
    }

    /// Test that the pool can recover workers when their jobs panic.  Submit two jobs that panic,
    /// and then submit MAX_THREADS jobs, each that sleep a descending number of milliseconds such
    /// that the pool will be saturated. Verify that there are still 4 threads in play.
    #[test]
    fn test_panic_recovery() -> Result<(), Box<dyn std::error::Error>> {
        let max_threads = 4;
        let thread_pool = ThreadPoolBuilder::new().with_size(max_threads).build()?;

        let executor = thread_pool.executor();

        executor.execute(|| panic!("first panicking!"));
        executor.execute(|| panic!("second panicking!"));

        // verify that we still have `max_threads`
        let (tx, rx) = channel();
        let barrier = Arc::new(Barrier::new(max_threads));
        for _ in 0..max_threads {
            let job_tx = tx.clone();
            let barrier = Arc::clone(&barrier);
            executor.execute(move || {
                job_tx
                    .send(
                        thread::current()
                            .name()
                            .expect("worker thread was not named")
                            .to_string(),
                    )
                    .expect("Unable to send result");

                // ensures that every worker is in use by the time the last job is run
                barrier.wait();
            });
        }
        // Drop the sender so that the receiver will close when the last job is done.
        drop(tx);

        let mut results: Vec<_> = rx.iter().collect();

        results.sort();

        assert_eq!(
            vec![
                "ThreadPool-0".to_string(),
                "ThreadPool-1".to_string(),
                "ThreadPool-2".to_string(),
                "ThreadPool-3".to_string()
            ],
            results
        );

        thread_pool.shutdown_signaler().shutdown();

        thread_pool.join_all();

        Ok(())
    }
}
