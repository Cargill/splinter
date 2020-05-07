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

use std::any::Any;
use std::error::Error;
use std::fmt;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::mpsc::{channel, Receiver, RecvError, Sender};

use super::{Dispatcher, PeerId};

/// A message to be dispatched.
///
/// This enum contains information about a message that will be passed to a `Dispatcher` instance
/// via a `Sender<DispatchMessage>`.
enum DispatchMessage<MT, Source = PeerId>
where
    MT: Any + Hash + Eq + Debug + Clone,
{
    Message {
        message_type: MT,
        message_bytes: Vec<u8>,
        source_id: Source,
        parent_context: Option<Box<dyn Any + Send>>,
    },
    Shutdown,
}

#[derive(Clone)]
pub struct DispatchLoopShutdownSignaler<MT, Source = PeerId>
where
    MT: Any + Hash + Eq + Debug + Clone,
{
    sender: Sender<DispatchMessage<MT, Source>>,
}

impl<MT, Source> DispatchLoopShutdownSignaler<MT, Source>
where
    MT: Any + Hash + Eq + Debug + Clone,
{
    pub fn shutdown(&self) {
        if self.sender.send(DispatchMessage::Shutdown).is_err() {
            error!("Unable to send shutdown signal to already shutdown dispatch loop");
        }
    }
}

/// Errors that may occur during the operation of the Dispatch Loop.
#[derive(Debug)]
pub struct DispatchLoopError(String);

impl Error for DispatchLoopError {}

impl fmt::Display for DispatchLoopError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "received error from dispatch loop: {}", self.0)
    }
}

#[derive(Default)]
pub struct DispatchLoopBuilder<MT, Source = PeerId>
where
    Source: 'static,
    MT: Any + Hash + Eq + Debug + Clone,
{
    dispatcher: Option<Dispatcher<MT, Source>>,
    channel: Option<(
        DispatchMessageSender<MT, Source>,
        DispatchMessageReceiver<MT, Source>,
    )>,
    thread_name: Option<String>,
}

impl<MT, Source> DispatchLoopBuilder<MT, Source>
where
    MT: Any + Hash + Eq + Debug + Clone + Send,
    Source: Send + 'static,
{
    pub fn new() -> Self {
        DispatchLoopBuilder {
            dispatcher: None,
            channel: None,
            thread_name: None,
        }
    }

    pub fn with_dispatch_channel(
        mut self,
        channel: (
            DispatchMessageSender<MT, Source>,
            DispatchMessageReceiver<MT, Source>,
        ),
    ) -> Self {
        self.channel = Some(channel);
        self
    }

    pub fn with_dispatcher(mut self, dispatcher: Dispatcher<MT, Source>) -> Self {
        self.dispatcher = Some(dispatcher);
        self
    }

    pub fn with_thread_name(mut self, name: String) -> Self {
        self.thread_name = Some(name);
        self
    }

    pub fn build(mut self) -> Result<DispatchLoop<MT, Source>, String> {
        let (tx, rx) = self.channel.take().unwrap_or_else(dispatch_channel);

        let dispatcher = self
            .dispatcher
            .take()
            .ok_or_else(|| "No dispatch provided".to_string())?;

        let thread_name = self
            .thread_name
            .unwrap_or_else(|| format!("DispatchLoop({})", std::any::type_name::<MT>()));

        let join_handle = std::thread::Builder::new()
            .name(thread_name)
            .spawn(move || loop {
                match rx.receiver.recv() {
                    Ok(DispatchMessage::Message {
                        message_type,
                        message_bytes,
                        source_id,
                        parent_context: Some(context),
                    }) => {
                        if let Err(err) = dispatcher.dispatch_with_parent_context(
                            source_id,
                            &message_type,
                            message_bytes,
                            context,
                        ) {
                            warn!("Unable to dispatch message: {:?}", err);
                        }
                    }
                    Ok(DispatchMessage::Message {
                        message_type,
                        message_bytes,
                        source_id,
                        parent_context: None,
                    }) => {
                        if let Err(err) =
                            dispatcher.dispatch(source_id, &message_type, message_bytes)
                        {
                            warn!("Unable to dispatch message: {:?}", err);
                        }
                    }
                    Ok(DispatchMessage::Shutdown) => {
                        debug!("Received shutdown signal");
                        break;
                    }
                    Err(RecvError) => {
                        error!("Received error from receiver");
                        break;
                    }
                }
            });

        match join_handle {
            Ok(join_handle) => Ok(DispatchLoop {
                sender: tx.sender,
                join_handle,
            }),
            Err(err) => Err(format!("Unable to start up dispatch loop thread: {}", err)),
        }
    }
}

/// The Dispatch Loop
///
/// The dispatch loop processes messages that are pulled from a `Receiver<DispatchMessage>` and
/// passes them to a Dispatcher.  The dispatch loop only processes messages from a specific message
/// type.
pub struct DispatchLoop<MT, Source = PeerId>
where
    MT: Any + Hash + Eq + Debug + Clone,
{
    sender: Sender<DispatchMessage<MT, Source>>,
    join_handle: std::thread::JoinHandle<()>,
}

impl<MT, Source> DispatchLoop<MT, Source>
where
    MT: Any + Hash + Eq + Debug + Clone,
{
    pub fn wait_for_shutdown(self) {
        if self.join_handle.join().is_err() {
            error!("Unable to cleanly wait for dispatch loop shutdown");
        }
    }

    pub fn new_dispatcher_sender(&self) -> DispatchMessageSender<MT, Source> {
        DispatchMessageSender {
            sender: self.sender.clone(),
        }
    }

    pub fn shutdown_signaler(&self) -> DispatchLoopShutdownSignaler<MT, Source> {
        DispatchLoopShutdownSignaler {
            sender: self.sender.clone(),
        }
    }
}

pub fn dispatch_channel<MT, Source>() -> (
    DispatchMessageSender<MT, Source>,
    DispatchMessageReceiver<MT, Source>,
)
where
    MT: Any + Hash + Eq + Debug + Clone,
{
    let (tx, rx) = channel();
    (
        DispatchMessageSender { sender: tx },
        DispatchMessageReceiver { receiver: rx },
    )
}

pub struct DispatchMessageReceiver<MT, Source = PeerId>
where
    MT: Any + Hash + Eq + Debug + Clone,
{
    receiver: Receiver<DispatchMessage<MT, Source>>,
}

// These type defs make clippy happy.
type MessageTuple<MT, Source> = (MT, Vec<u8>, Source);
type MessageTupleWithParentContext<MT, Source> = (MT, Vec<u8>, Source, Box<dyn Any + Send>);

#[derive(Clone)]
pub struct DispatchMessageSender<MT, Source = PeerId>
where
    MT: Any + Hash + Eq + Debug + Clone,
{
    sender: Sender<DispatchMessage<MT, Source>>,
}

impl<MT, Source> DispatchMessageSender<MT, Source>
where
    MT: Any + Hash + Eq + Debug + Clone,
{
    pub fn send(
        &self,
        message_type: MT,
        message_bytes: Vec<u8>,
        source_id: Source,
    ) -> Result<(), MessageTuple<MT, Source>> {
        self.sender
            .send(DispatchMessage::Message {
                message_type,
                message_bytes,
                source_id,
                parent_context: None,
            })
            .map_err(|err| match err.0 {
                DispatchMessage::Message {
                    message_type,
                    message_bytes,
                    source_id,
                    ..
                } => (message_type, message_bytes, source_id),
                DispatchMessage::Shutdown => unreachable!(), // we didn't send this
            })
    }

    pub fn send_with_parent_context(
        &self,
        message_type: MT,
        message_bytes: Vec<u8>,
        source_id: Source,
        parent_context: Box<dyn Any + Send>,
    ) -> Result<(), MessageTupleWithParentContext<MT, Source>> {
        self.sender
            .send(DispatchMessage::Message {
                message_type,
                message_bytes,
                source_id,
                parent_context: Some(parent_context),
            })
            .map_err(|err| match err.0 {
                DispatchMessage::Message {
                    message_type,
                    message_bytes,
                    source_id,
                    parent_context: Some(pc),
                } => (message_type, message_bytes, source_id, pc),
                _ => unreachable!(), // we didn't anything else
            })
    }
}
