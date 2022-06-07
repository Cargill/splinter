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

use crate::rest_api::actix_web_1::Method as Actix1Method;

#[derive(PartialEq, Clone)]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
}

impl From<&Actix1Method> for Method {
    fn from(source: &Actix1Method) -> Self {
        match source {
            Actix1Method::Get => Method::Get,
            Actix1Method::Post => Method::Post,
            Actix1Method::Put => Method::Put,
            Actix1Method::Patch => Method::Patch,
            Actix1Method::Delete => Method::Delete,
            Actix1Method::Head => Method::Head,
        }
    }
}

impl From<Actix1Method> for Method {
    fn from(source: crate::rest_api::actix_web_1::Method) -> Self {
        (&source).into()
    }
}
