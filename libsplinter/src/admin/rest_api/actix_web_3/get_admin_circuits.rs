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

//! This module provides the `GET /admin/circuits` endpoint for listing the definitions of circuits
//! in Splinter's state.

use actix_web_3::{HttpResponse, Responder};

/// An implementation of the `GET /admin/circuits` endpoint.
pub async fn get_admin_circuits() -> impl Responder {
    HttpResponse::Ok().body("hello world")
}
