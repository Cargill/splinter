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

use actix_web::{Error as ActixError, HttpRequest, HttpResponse};
use futures::{Future, IntoFuture};

/// A continuation indicates whether or not a guard should allow a given request to continue, or to
/// return a result.
pub enum Continuation {
    Continue,
    Terminate(Box<dyn Future<Item = HttpResponse, Error = ActixError>>),
}

impl Continuation {
    /// Wraps the given future in the Continuation::Terminate variant.
    pub fn terminate<F>(fut: F) -> Continuation
    where
        F: Future<Item = HttpResponse, Error = ActixError> + 'static,
    {
        Continuation::Terminate(Box::new(fut))
    }
}

/// A guard checks the request content in advance, and either continues the request, or
/// returns a terminating result.
pub trait RequestGuard: Send + Sync {
    /// Evaluates the request and determines whether or not the request should be continued or
    /// short-circuited with a terminating future.
    fn evaluate(&self, req: &HttpRequest) -> Continuation;
}

impl<F> RequestGuard for F
where
    F: Fn(&HttpRequest) -> Continuation + Sync + Send,
{
    fn evaluate(&self, req: &HttpRequest) -> Continuation {
        (*self)(req)
    }
}

impl RequestGuard for Box<dyn RequestGuard> {
    fn evaluate(&self, req: &HttpRequest) -> Continuation {
        (**self).evaluate(req)
    }
}

/// Guards requests based on a minimum protocol version.
///
/// A protocol version is specified via the HTTP header `"SplinterProtocolVersion"`.  This header
/// is a positive integer value.
#[derive(Clone)]
pub struct ProtocolVersionRangeGuard {
    min: u32,
    max: u32,
}

impl ProtocolVersionRangeGuard {
    /// Constructs a new protocol version guard with the given minimum.
    pub fn new(min: u32, max: u32) -> Self {
        Self { min, max }
    }
}

impl RequestGuard for ProtocolVersionRangeGuard {
    fn evaluate(&self, req: &HttpRequest) -> Continuation {
        if let Some(header_value) = req.headers().get("SplinterProtocolVersion") {
            let parsed_header = header_value
                .to_str()
                .map_err(|err| {
                    format!(
                        "Invalid characters in SplinterProtocolVersion header: {}",
                        err
                    )
                })
                .and_then(|val_str| {
                    val_str.parse::<u32>().map_err(|_| {
                        "SplinterProtocolVersion must be a valid positive integer".to_string()
                    })
                });
            match parsed_header {
                Err(msg) => Continuation::terminate(
                    HttpResponse::BadRequest()
                        .json(json!({
                            "message": msg,
                        }))
                        .into_future(),
                ),
                Ok(version) if version < self.min => Continuation::terminate(
                    HttpResponse::BadRequest()
                        .json(json!({
                            "message": format!(
                                "Client must support protocol version {} or greater.",
                                self.min,
                            ),
                            "requested_protocol": version,
                            "splinter_protocol": self.max,
                            "libsplinter_version": format!(
                                "{}.{}.{}",
                                env!("CARGO_PKG_VERSION_MAJOR"),
                                env!("CARGO_PKG_VERSION_MINOR"),
                                env!("CARGO_PKG_VERSION_PATCH")
                            )
                        }))
                        .into_future(),
                ),
                Ok(version) if version > self.max => Continuation::terminate(
                    HttpResponse::BadRequest()
                        .json(json!({
                            "message": format!(
                                "Client requires a newer protocol than can be provided: {} > {}",
                                version,
                                self.max,
                            ),
                            "requested_protocol": version,
                            "splinter_protocol": self.max,
                            "libsplinter_version": format!(
                                "{}.{}.{}",
                                env!("CARGO_PKG_VERSION_MAJOR"),
                                env!("CARGO_PKG_VERSION_MINOR"),
                                env!("CARGO_PKG_VERSION_PATCH")
                            )
                        }))
                        .into_future(),
                ),
                Ok(_) => Continuation::Continue,
            }
        } else {
            // Ignore the missing header, and assume the client will handle version mismatches by
            // inspecting the output
            Continuation::Continue
        }
    }
}
