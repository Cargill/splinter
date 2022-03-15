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

//!  Splinter services are a fundamental component of a Splinter network.  They provide the
//!  application-level business logic, abstracted above the underlying network and circuit layers.
//!  The Service API provides a framework for defining services from a Splinter perspective.
//!
//!  Splinter services are effectively message handlers.  They receive the bytes of a message and a
//!  context for the message (including the originating sender).  It is up to the service to parse
//!  the message bytes into the format desired.
//!
//!  A service is identified by an ID, which must be unique within a Splinter circuit.  It also
//!  provides a service type, which indicates what kinds of capabilities the service provides.
//!
//!  There may be more than one service of a given service type on a circuit, though each instance
//!  must continue to have a unique ID.
//!
//!  Services are started and stopped explicitly.  At these times, the service must either register
//!  (at start) or unregister (at stop) itself.  In splinter terms, these two operations connect
//!  and disconnect the service from a circuit, but the Service API keeps the service circuit-
//!  agnostic.
//!
//!  A stand-alone service implementation may be wrapped in a ServiceProcessor, which will manage
//!  lower-level messaging and networking needs to talk to applications that implement Splinter
//!  node capabilities, such as the Splinter daemon.

#[cfg(feature = "service-arguments-converter")]
mod arguments_converter;
#[cfg(feature = "service-id")]
mod id;
pub mod instance;
#[cfg(feature = "service-lifecycle")]
mod lifecycle;
#[cfg(feature = "service-message-converter")]
mod message_converter;
#[cfg(feature = "service-message-handler")]
mod message_handler;
#[cfg(feature = "service-message-handler-factory")]
mod message_handler_factory;
#[cfg(feature = "service-message-sender")]
mod message_sender;
#[cfg(feature = "service-message-sender-factory")]
mod message_sender_factory;
#[cfg(feature = "rest-api-actix-web-1")]
pub mod rest_api;
#[cfg(feature = "service-routable")]
mod routable;
#[cfg(feature = "service-timer-filter")]
mod timer_filter;
#[cfg(feature = "service-timer-handler")]
mod timer_handler;
#[cfg(feature = "service-timer-handler-factory")]
mod timer_handler_factory;

#[cfg(feature = "service-arguments-converter")]
pub use arguments_converter::ArgumentsConverter;
#[cfg(feature = "service-id")]
pub use id::{CircuitId, FullyQualifiedServiceId, ServiceId};
#[cfg(feature = "service-lifecycle")]
pub use lifecycle::Lifecycle;
#[cfg(feature = "service-message-converter")]
pub use message_converter::MessageConverter;
#[cfg(feature = "service-message-handler")]
pub use message_handler::MessageHandler;
#[cfg(feature = "service-message-handler-factory")]
pub use message_handler_factory::MessageHandlerFactory;
#[cfg(feature = "service-message-sender")]
use message_sender::IntoMessageSender;
#[cfg(feature = "service-message-sender")]
pub use message_sender::MessageSender;
#[cfg(feature = "service-message-sender-factory")]
pub use message_sender_factory::MessageSenderFactory;
#[cfg(feature = "service-routable")]
pub use routable::{Routable, Typed};
#[cfg(feature = "service-timer-filter")]
pub use timer_filter::TimerFilter;
#[cfg(feature = "service-timer-handler")]
pub use timer_handler::TimerHandler;
#[cfg(feature = "service-timer-handler-factory")]
pub use timer_handler_factory::TimerHandlerFactory;
