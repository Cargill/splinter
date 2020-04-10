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

//! Provides CORS support for the REST API
//!
//! This is an experimental feature.  To enable, use the feature `"rest-api-cors"`.
use actix_web::dev::*;
use actix_web::{
    http::header, http::header::HeaderValue, http::Method, Error as ActixError, HttpResponse,
};
use futures::{
    future::{ok, FutureResult},
    Future, IntoFuture, Poll,
};

/// Configuration for CORS support
#[derive(Clone)]
pub struct Cors {
    whitelist: Vec<String>,
}

impl Cors {
    /// Initialize the CORS preflight check with a set of allowed domains.
    pub fn new(whitelist: Vec<String>) -> Self {
        Cors { whitelist }
    }

    /// Initialize the CORS preflight check with "*" domains.
    pub fn new_allow_any() -> Self {
        Cors::new(vec!["*".into()])
    }
}

impl<S, B> Transform<S> for Cors
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = ActixError>,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = S::Error;
    type InitError = ();
    type Transform = CorsMiddleware<S>;
    type Future = FutureResult<Self::Transform, Self::InitError>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(CorsMiddleware {
            service,
            whitelist: self.whitelist.clone(),
        })
    }
}

#[doc(hidden)]
pub struct CorsMiddleware<S> {
    service: S,
    whitelist: Vec<String>,
}

impl<S, B> Service for CorsMiddleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = ActixError>,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = S::Error;
    type Future = Box<dyn Future<Item = Self::Response, Error = Self::Error>>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        self.service.poll_ready()
    }

    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        let origin_header = req.headers().get(header::ORIGIN).cloned();
        let origin: Result<Option<String>, _> = origin_header
            .as_ref()
            .map(|h| h.to_str().map(String::from))
            .transpose();

        match (origin, origin_header) {
            (Ok(Some(origin)), Some(origin_header)) => {
                let request_headers = req
                    .headers()
                    .get(header::ACCESS_CONTROL_REQUEST_HEADERS)
                    .cloned();
                let allowed_origin = self
                    .whitelist
                    .iter()
                    .any(|domain| domain == "*" || origin.contains(domain));
                // This verifies if a client is making a preflight check with the OPTIONS
                // http request method and the origin is allowed, the preflight check responds
                // with a 200 OK status.
                if allowed_origin && req.method() == Method::OPTIONS {
                    debug!("Preflight check passed");
                    let mut res = req.into_response(HttpResponse::Ok().finish().into_body());
                    let headers = res.headers_mut();
                    headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin_header);
                    headers.insert(
                        header::ACCESS_CONTROL_ALLOW_METHODS,
                        HeaderValue::from_static("*"),
                    );
                    headers.insert(
                        header::ACCESS_CONTROL_ALLOW_HEADERS,
                        request_headers.unwrap_or_else(|| HeaderValue::from_static("*")),
                    );
                    Box::new(res.into_future())
                } else if allowed_origin {
                    Box::new(self.service.call(req).map(move |mut res| {
                        let headers = res.headers_mut();
                        headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin_header);
                        headers.insert(
                            header::ACCESS_CONTROL_ALLOW_METHODS,
                            HeaderValue::from_static("*"),
                        );
                        headers.insert(
                            header::ACCESS_CONTROL_ALLOW_HEADERS,
                            request_headers.unwrap_or_else(|| HeaderValue::from_static("*")),
                        );
                        res
                    }))
                } else {
                    Box::new(
                        req.into_response(HttpResponse::PreconditionFailed().finish().into_body())
                            .into_future(),
                    )
                }
            }
            (Ok(Some(_)), None) => unreachable!(),
            (Ok(None), _) => Box::new(self.service.call(req)),
            (Err(_), _) => Box::new(
                req.into_response(HttpResponse::BadRequest().finish().into_body())
                    .into_future(),
            ),
        }
    }
}
