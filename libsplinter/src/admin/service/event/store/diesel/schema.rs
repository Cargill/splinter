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

table! {
    admin_service_event (id) {
        id -> Int8,
        event_type -> Text,
        data -> Nullable<Binary>,
    }
}

table! {
    admin_event_circuit_proposal (event_id) {
        event_id -> Int8,
        proposal_type -> Text,
        circuit_id -> Text,
        circuit_hash -> Text,
        requester -> Binary,
        requester_node_id -> Text,
    }
}

table! {
    admin_event_proposed_circuit (event_id) {
        event_id -> Int8,
        circuit_id -> Text,
        authorization_type -> Text,
        persistence -> Text,
        durability -> Text,
        routes -> Text,
        circuit_management_type -> Text,
        application_metadata -> Nullable<Binary>,
        comments -> Nullable<Text>,
        display_name -> Nullable<Text>,
    }
}

table! {
    admin_event_vote_record (event_id, voter_node_id) {
        event_id -> Int8,
        public_key -> Binary,
        vote -> Text,
        voter_node_id -> Text,
    }
}

table! {
    admin_event_proposed_node (event_id, node_id) {
        event_id -> Int8,
        node_id -> Text,
    }
}

table! {
    admin_event_proposed_node_endpoint (event_id, node_id, endpoint) {
        event_id -> Int8,
        node_id -> Text,
        endpoint -> Text,
    }
}

table! {
    admin_event_proposed_service (event_id, service_id) {
        event_id -> Int8,
        service_id -> Text,
        service_type -> Text,
        node_id -> Text,
    }
}

table! {
    admin_event_proposed_service_argument (event_id, service_id, key) {
        event_id -> Int8,
        service_id -> Text,
        key -> Text,
        value -> Text,
    }
}

allow_tables_to_appear_in_same_query!(
    admin_service_event,
    admin_event_proposed_circuit,
    admin_event_proposed_node,
    admin_event_proposed_node_endpoint,
    admin_event_proposed_service,
    admin_event_proposed_service_argument,
    admin_event_vote_record,
    admin_event_circuit_proposal,
);
