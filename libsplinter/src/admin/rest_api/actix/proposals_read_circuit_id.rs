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

//! Provides the `GET /admin/proposals/{circuit_id} endpoint for fetching circuit proposals by
//! circuit ID.

use actix_web::{error::BlockingError, web, Error, HttpRequest, HttpResponse};
use futures::executor::block_on;

use crate::admin::messages::CircuitProposal;
use crate::admin::rest_api::error::ProposalFetchError;
use crate::admin::service::proposal_store::ProposalStore;
use crate::protocol;
use crate::rest_api::{Method, ProtocolVersionRangeGuard, Resource};

pub fn make_fetch_proposal_resource<PS: ProposalStore + 'static>(proposal_store: PS) -> Resource {
    Resource::build("admin/proposals/{circuit_id}")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::ADMIN_FETCH_PROPOSALS_PROTOCOL_MIN,
            protocol::ADMIN_PROTOCOL_VERSION,
        ))
        .add_method(Method::Get, move |r, _| {
            block_on(fetch_proposal(r, web::Data::new(proposal_store.clone())))
        })
}

async fn fetch_proposal<PS: ProposalStore + Clone + 'static>(
    request: HttpRequest,
    proposal_store: web::Data<PS>,
) -> Result<HttpResponse, Error> {
    let circuit_id = request
        .match_info()
        .get("circuit_id")
        .unwrap_or("")
        .to_string();

    match web::block(move || {
        let proposal = proposal_store
            .proposal(&circuit_id)
            .map_err(|err| ProposalFetchError::InternalError(err.to_string()))?;
        if let Some(proposal) = proposal {
            let proposal = CircuitProposal::from_proto(proposal)
                .map_err(|err| ProposalFetchError::InternalError(err.to_string()))?;

            Ok(proposal)
        } else {
            Err(ProposalFetchError::NotFound(format!(
                "Unable to find proposal: {}",
                circuit_id
            )))
        }
    })
    .await
    {
        Ok(proposal) => Ok(HttpResponse::Ok().json(proposal)),
        Err(err) => match err {
            BlockingError::Error(err) => match err {
                ProposalFetchError::InternalError(_) => {
                    error!("{}", err);
                    Ok(HttpResponse::InternalServerError().into())
                }
                ProposalFetchError::NotFound(err) => Ok(HttpResponse::NotFound().json(err)),
            },
            _ => Ok(HttpResponse::InternalServerError().into()),
        },
    }
}
