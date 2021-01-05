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

mod api;
mod auth;
mod builder;
mod error;
mod guard;
mod resource;
mod websocket;

pub use api::{RestApi, RestApiShutdownHandle};
#[cfg(feature = "auth")]
pub use auth::AuthConfig;
pub use auth::{get_authorization_token, require_header};
pub use builder::RestApiBuilder;
pub use error::ResponseError;
pub use guard::{Continuation, ProtocolVersionRangeGuard, RequestGuard};
pub use resource::{
    into_bytes, into_protobuf, HandlerFunction, Method, Resource, RestResourceProvider,
};
pub use websocket::{new_websocket_event_sender, EventSender, Request, Response};
