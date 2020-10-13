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
    circuit_proposal (circuit_id) {
        proposal_type -> Text,
        circuit_id -> Text,
        circuit_hash -> Text,
        requester -> Binary,
        requester_node_id -> Text,
    }
}

table! {
    proposed_circuit (circuit_id) {
        circuit_id -> Text,
        authorization_type -> Text,
        persistence -> Text,
        durability -> Text,
        routes -> Text,
        circuit_management_type -> Text,
        application_metadata -> Binary,
        comments -> Text,
    }
}

table! {
    vote_record (circuit_id, voter_node_id) {
        circuit_id -> Text,
        public_key -> Binary,
        vote -> Text,
        voter_node_id -> Text,
    }
}

table! {
    proposed_node (circuit_id, node_id) {
        circuit_id -> Text,
        node_id -> Text,
    }
}

table! {
    proposed_node_endpoint (node_id, endpoint) {
        node_id -> Text,
        endpoint -> Text,
    }
}

table! {
    proposed_service (circuit_id, service_id) {
        circuit_id -> Text,
        service_id -> Text,
        service_type -> Text,
        node_id -> Text,
    }
}

table! {
    proposed_service_argument (circuit_id, service_id, key) {
        circuit_id -> Text,
        service_id -> Text,
        key -> Text,
        value -> Text,
    }
}

table! {
    service (circuit_id, service_id) {
        circuit_id -> Text,
        service_id -> Text,
        service_type -> Text,
        node_id -> Text,
    }
}

table! {
    service_argument (circuit_id, service_id, key) {
        circuit_id -> Text,
        service_id -> Text,
        key -> Text,
        value -> Text,
    }
}

table! {
    circuit (circuit_id) {
        circuit_id -> Text,
        authorization -> Text,
        persistence -> Text,
        durability -> Text,
        routes -> Text,
        circuit_management_type -> Text,
    }
}

table! {
    circuit_member (circuit_id, node_id) {
        circuit_id -> Text,
        node_id -> Text,
    }
}

table! {
    node_endpoint (node_id, endpoint) {
        node_id -> Text,
        endpoint -> Text,
    }
}

allow_tables_to_appear_in_same_query!(
    proposed_circuit,
    proposed_node,
    proposed_node_endpoint,
    proposed_service,
    proposed_service_argument,
    vote_record,
    circuit_proposal,
);

allow_tables_to_appear_in_same_query!(
    service,
    service_argument,
    circuit,
    circuit_member,
    node_endpoint
);
