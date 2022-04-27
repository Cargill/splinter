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
    scabbard_service (service_id) {
        service_id  -> Text,
        consensus -> Text,
        status -> Text,
    }
}

table! {
    scabbard_peer (service_id, peer_service_id) {
        service_id  -> Text,
        peer_service_id  -> Text,
    }
}

table! {
    scabbard_v3_commit_history (service_id, epoch) {
        service_id  -> Text,
        epoch -> BigInt,
        value -> VarChar,
        decision -> Nullable<Text>,
    }
}

table! {
    consensus_2pc_consensus_coordinator_context (service_id, epoch) {
        service_id -> Text,
        alarm -> Nullable<BigInt>,
        coordinator -> Text,
        epoch -> BigInt,
        last_commit_epoch -> Nullable<BigInt>,
        state -> Text,
        vote_timeout_start -> Nullable<BigInt>,
    }
}

table! {
    consensus_2pc_consensus_coordinator_context_participant (service_id, epoch, process) {
        service_id -> Text,
        epoch -> BigInt,
        process -> Text,
        vote -> Nullable<Text>,
    }
}

table! {
    consensus_2pc_coordinator_notification_action (action_id) {
        action_id -> Int8,
        service_id -> Text,
        epoch -> BigInt,
        notification_type -> Text,
        dropped_message -> Nullable<Text>,
    }
}

table! {
    consensus_2pc_coordinator_send_message_action (action_id) {
        action_id -> Int8,
        service_id -> Text,
        epoch -> BigInt,
        receiver_service_id -> Text,
        message_type -> Text,
        vote_response -> Nullable<Text>,
    }
}

table! {
    consensus_2pc_update_coordinator_context_action (action_id) {
        action_id -> Int8,
        service_id -> Text,
        alarm -> Nullable<BigInt>,
        coordinator -> Text,
        epoch -> BigInt,
        last_commit_epoch -> Nullable<BigInt>,
        state -> Text,
        vote_timeout_start -> Nullable<BigInt>,
        coordinator_action_alarm -> Nullable<BigInt>,
    }
}

table! {
    consensus_2pc_update_coordinator_context_action_participant (action_id) {
        action_id -> Int8,
        service_id -> Text,
        epoch -> BigInt,
        process -> Text,
        vote -> Nullable<Text>,
    }
}

table! {
    consensus_2pc_action (id) {
        id -> Int8,
        service_id -> Text,
        epoch -> BigInt,
        created_at -> Timestamp,
        executed_at -> Nullable<BigInt>,
        position -> Integer,
    }
}

table! {
    consensus_2pc_participant_context (service_id, epoch) {
        service_id -> Text,
        alarm -> Nullable<BigInt>,
        coordinator -> Text,
        epoch -> BigInt,
        last_commit_epoch -> Nullable<BigInt>,
        state -> Text,
        vote -> Nullable<Text>,
        decision_timeout_start -> Nullable<BigInt>,
    }
}

table! {
    consensus_2pc_participant_context_participant (service_id, epoch, process) {
        service_id -> Text,
        epoch -> BigInt,
        process -> Text,
    }
}

table! {
    consensus_2pc_participant_notification_action (action_id) {
        action_id -> Int8,
        service_id -> Text,
        epoch -> BigInt,
        notification_type -> Text,
        dropped_message -> Nullable<Text>,
        request_for_vote_value -> Nullable<Binary>,
    }
}

table! {
    consensus_2pc_participant_send_message_action (action_id) {
        action_id -> Int8,
        service_id -> Text,
        epoch -> BigInt,
        receiver_service_id -> Text,
        message_type -> Text,
        vote_request -> Nullable<Binary>,
    }
}

table! {
    consensus_2pc_update_participant_context_action (action_id) {
        action_id -> Int8,
        service_id -> Text,
        alarm -> Nullable<BigInt>,
        coordinator -> Text,
        epoch -> BigInt,
        last_commit_epoch -> Nullable<BigInt>,
        state -> Text,
        vote -> Nullable<Text>,
        decision_timeout_start -> Nullable<BigInt>,
        participant_action_alarm -> Nullable<BigInt>,
    }
}

table! {
    consensus_2pc_update_participant_context_action_participant (action_id) {
        action_id -> Int8,
        service_id -> Text,
        epoch -> BigInt,
        process -> Text,
    }
}

table! {
    two_pc_consensus_event (id) {
        id -> Int8,
        service_id -> Text,
        epoch -> BigInt,
        created_at -> Timestamp,
        executed_at -> Nullable<BigInt>,
        position -> Integer,
        event_type -> Text,
    }
}

table! {
    two_pc_consensus_deliver_event (event_id) {
        event_id -> Int8,
        service_id -> Text,
        epoch -> BigInt,
        receiver_service_id -> Text,
        message_type -> Text,
        vote_response -> Nullable<Text>,
        vote_request -> Nullable<Binary>,
    }
}

table! {
    two_pc_consensus_start_event (event_id) {
        event_id -> Int8,
        service_id -> Text,
        epoch -> BigInt,
        value -> Binary,
    }
}

table! {
    two_pc_consensus_vote_event (event_id) {
        event_id -> Int8,
        service_id -> Text,
        epoch -> BigInt,
        vote -> Text,
    }
}

joinable!(consensus_2pc_coordinator_notification_action -> consensus_2pc_action (action_id));
joinable!(consensus_2pc_coordinator_send_message_action -> consensus_2pc_action (action_id));
joinable!(consensus_2pc_update_coordinator_context_action -> consensus_2pc_action (action_id));
joinable!(consensus_2pc_update_coordinator_context_action_participant -> consensus_2pc_update_coordinator_context_action (action_id));
joinable!(consensus_2pc_update_coordinator_context_action_participant -> consensus_2pc_action (action_id));

joinable!(consensus_2pc_participant_notification_action -> consensus_2pc_action (action_id));
joinable!(consensus_2pc_participant_send_message_action -> consensus_2pc_action (action_id));
joinable!(consensus_2pc_update_participant_context_action -> consensus_2pc_action (action_id));

joinable!(two_pc_consensus_deliver_event -> two_pc_consensus_event(event_id));
joinable!(two_pc_consensus_start_event -> two_pc_consensus_event(event_id));
joinable!(two_pc_consensus_vote_event -> two_pc_consensus_event(event_id));

allow_tables_to_appear_in_same_query!(
    consensus_2pc_consensus_coordinator_context,
    consensus_2pc_consensus_coordinator_context_participant,
    consensus_2pc_action,
    consensus_2pc_update_coordinator_context_action,
    consensus_2pc_coordinator_send_message_action,
    consensus_2pc_coordinator_notification_action,
    consensus_2pc_update_coordinator_context_action_participant,
    consensus_2pc_participant_context,
    consensus_2pc_participant_context_participant,
    consensus_2pc_update_participant_context_action,
    consensus_2pc_update_participant_context_action_participant,
    consensus_2pc_participant_send_message_action,
    consensus_2pc_participant_notification_action,
    two_pc_consensus_event,
    two_pc_consensus_deliver_event,
    two_pc_consensus_start_event,
    two_pc_consensus_vote_event,
);

allow_tables_to_appear_in_same_query!(scabbard_peer, scabbard_service, scabbard_v3_commit_history,);
