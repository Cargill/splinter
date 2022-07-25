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

table! {
    echo_peers (service_id, peer_service_id) {
        service_id -> Text,
        peer_service_id -> Nullable<Text>,
    }
}

table! {
    echo_request_errors (service_id, correlation_id) {
        service_id -> Text,
        correlation_id -> BigInt,
        error_message -> Text,
        error_at -> BigInt,
    }
}

table! {
    echo_requests (sender_service_id, correlation_id) {
        sender_service_id -> Text,
        correlation_id -> BigInt,
        receiver_service_id -> Text,
        message -> Text,
        sent -> SmallInt,
        sent_at -> Nullable<BigInt>,
        ack ->  SmallInt,
        ack_at -> Nullable<BigInt>,
    }
}

table! {
    echo_services (service_id) {
        service_id -> Text,
        frequency -> Nullable<BigInt>,
        jitter -> Nullable<BigInt>,
        error_rate -> Nullable<Float>,
        status -> SmallInt,
    }
}

joinable!(echo_peers -> echo_services (service_id));
joinable!(echo_requests -> echo_services (sender_service_id));

allow_tables_to_appear_in_same_query!(
    echo_peers,
    echo_request_errors,
    echo_requests,
    echo_services,
);
