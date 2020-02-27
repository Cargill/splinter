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

use std::{cell::RefCell, pin::Pin, rc::Rc, task::Context};

use actix_web::dev::*;
use actix_web::{http::header, http::header::HeaderValue, Error as ActixError, HttpResponse};
use futures::{
    future::{ok, Future, Ready},
    task::Poll,
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
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = ActixError>
        + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = ActixError;
    type InitError = ();
    type Transform = CorsMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(CorsMiddleware {
            service: Rc::new(RefCell::new(service)),
            whitelist: self.whitelist.clone(),
        })
    }
}

#[doc(hidden)]
pub struct CorsMiddleware<S> {
    service: Rc<RefCell<S>>,
    whitelist: Vec<String>,
}

impl<S, B> Service for CorsMiddleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = ActixError>
        + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = S::Error;

    #[allow(clippy::type_complexity)]
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, ct: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ct)
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
                if allowed_origin {
                    let mut svc = self.service.clone();
                    Box::pin(async move {
                        let mut res = svc.call(req).await?;

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

                        Ok(res)
                    })
                } else {
                    let response =
                        req.into_response(HttpResponse::PreconditionFailed().finish().into_body());

                    Box::pin(async { Ok(response) })
                }
            }
            (Ok(Some(_)), None) => unreachable!(),
            (Ok(None), _) => {
                let mut svc = self.service.clone();
                Box::pin(async move { svc.call(req).await })
            }
            (Err(_), _) => {
                let response = req.into_response(HttpResponse::BadRequest().finish().into_body());

                Box::pin(async { Ok(response) })
            }
        }
    }
}
