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

use splinter::{error::InternalError, service::FullyQualifiedServiceId, service::ServiceId};

use crate::service::{EchoArguments, EchoRequest, EchoServiceStatus, RequestStatus};

pub trait EchoStore {
    fn add_service(
        &self,
        service: &FullyQualifiedServiceId,
        arguments: &EchoArguments,
    ) -> Result<(), InternalError>;

    fn remove_service(&self, service: &FullyQualifiedServiceId) -> Result<(), InternalError>;

    fn get_service_arguments(
        &self,
        service: &FullyQualifiedServiceId,
    ) -> Result<EchoArguments, InternalError>;

    // returns the correlation id
    fn insert_request(
        &self,
        service: &FullyQualifiedServiceId,
        to_service: &ServiceId,
        message: &str,
    ) -> Result<u64, InternalError>;

    fn update_request_sent(
        &self,
        service: &FullyQualifiedServiceId,
        correlation_id: i64,
        sent: RequestStatus,
        sent_at: Option<i64>,
    ) -> Result<(), InternalError>;

    fn get_last_sent(
        &self,
        sender_service_id: &FullyQualifiedServiceId,
        receiver_service_id: &ServiceId,
    ) -> Result<Option<i64>, InternalError>;

    fn list_requests(
        &self,
        service: &FullyQualifiedServiceId,
        receiver_service_id: Option<&ServiceId>,
    ) -> Result<Vec<EchoRequest>, InternalError>;

    fn update_request_ack(
        &self,
        service: &FullyQualifiedServiceId,
        correlation_id: i64,
        ack: RequestStatus,
        ack_at: Option<i64>,
    ) -> Result<(), InternalError>;

    fn insert_request_error(
        &self,
        service: &FullyQualifiedServiceId,
        error_message: &str,
        error_at: i64,
    ) -> Result<u64, InternalError>;

    fn list_ready_services(&self) -> Result<Vec<FullyQualifiedServiceId>, InternalError>;

    fn update_service_status(
        &self,
        service: &FullyQualifiedServiceId,
        status: EchoServiceStatus,
    ) -> Result<(), InternalError>;

    fn get_service_status(
        &self,
        service: &FullyQualifiedServiceId,
    ) -> Result<EchoServiceStatus, InternalError>;
}
