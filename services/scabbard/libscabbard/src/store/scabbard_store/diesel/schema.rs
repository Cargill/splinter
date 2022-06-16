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
    scabbard_service (circuit_id, service_id) {
        circuit_id  -> Text,
        service_id  -> Text,
        consensus -> crate::store::scabbard_store::diesel::models::ConsensusTypeModelMapping,
        status -> crate::store::scabbard_store::diesel::models::ServiceStatusTypeModelMapping,
    }
}

table! {
    scabbard_peer (circuit_id, service_id, peer_service_id) {
        circuit_id  -> Text,
        service_id  -> Text,
        peer_service_id  -> Text,
    }
}

table! {
    scabbard_v3_commit_history (circuit_id, service_id, epoch) {
        circuit_id  -> Text,
        service_id  -> Text,
        epoch -> BigInt,
        value -> VarChar,
        decision ->
            Nullable<crate::store::scabbard_store::diesel::models::DecisionTypeModelMapping>,
    }
}

table! {
    scabbard_alarm (circuit_id, service_id, alarm_type) {
        circuit_id  -> Text,
        service_id -> Text,
        alarm_type -> crate::store::scabbard_store::diesel::models::AlarmTypeModelMapping,
        alarm -> BigInt,
    }
}

table! {
    consensus_2pc_context (circuit_id, service_id) {
        circuit_id  -> Text,
        service_id -> Text,
        coordinator -> Text,
        epoch -> BigInt,
        last_commit_epoch -> Nullable<BigInt>,
        state -> crate::store::scabbard_store::diesel::models::ContextStateModelMapping,
        vote_timeout_start -> Nullable<BigInt>,
        vote -> Nullable<Text>,
        decision_timeout_start -> Nullable<BigInt>,
        ack_timeout_start -> Nullable<BigInt>,
    }
}

table! {
    consensus_2pc_context_participant (circuit_id, service_id, process) {
        circuit_id  -> Text,
        service_id -> Text,
        epoch -> BigInt,
        process -> Text,
        vote -> Nullable<Text>,
        decision_ack -> Bool,
    }
}

table! {
    consensus_2pc_notification_action (action_id) {
        action_id -> Int8,
        notification_type -> crate::store::scabbard_store::diesel::models::NotificationTypeModelMapping,
        dropped_message -> Nullable<Text>,
        request_for_vote_value -> Nullable<Binary>,
    }
}

table! {
    consensus_2pc_send_message_action (action_id) {
        action_id -> Int8,
        epoch -> BigInt,
        receiver_service_id -> Text,
        message_type -> crate::store::scabbard_store::diesel::models::MessageTypeModelMapping,
        vote_response -> Nullable<Text>,
        vote_request -> Nullable<Binary>,
    }
}

table! {
    consensus_2pc_update_context_action (action_id) {
        action_id -> Int8,
        coordinator -> Text,
        epoch -> BigInt,
        last_commit_epoch -> Nullable<BigInt>,
        state -> crate::store::scabbard_store::diesel::models::ContextStateModelMapping,
        vote_timeout_start -> Nullable<BigInt>,
        vote -> Nullable<Text>,
        decision_timeout_start -> Nullable<BigInt>,
        action_alarm -> Nullable<BigInt>,
        ack_timeout_start -> Nullable<BigInt>,
    }
}

table! {
    consensus_2pc_update_context_action_participant (action_id) {
        action_id -> Int8,
        process -> Text,
        vote -> Nullable<Text>,
        decision_ack -> Bool,
    }
}

table! {
    consensus_2pc_action (id) {
        id -> Int8,
        circuit_id  -> Text,
        service_id -> Text,
        created_at -> Timestamp,
        executed_at -> Nullable<Timestamp>,
        action_type -> crate::store::scabbard_store::diesel::models::ActionTypeModelMapping,
        event_id -> Int8,
    }
}

table! {
    consensus_2pc_event (id) {
        id -> Int8,
        circuit_id  -> Text,
        service_id -> Text,
        created_at -> Timestamp,
        executed_at -> Nullable<Timestamp>,
        executed_epoch -> Nullable<BigInt>,
        position -> Integer,
        event_type -> crate::store::scabbard_store::diesel::models::EventTypeModelMapping,
    }
}

table! {
    consensus_2pc_deliver_event (event_id) {
        event_id -> Int8,
        epoch -> BigInt,
        receiver_service_id -> Text,
        message_type -> crate::store::scabbard_store::diesel::models::DeliverMessageTypeModelMapping,
        vote_response -> Nullable<Text>,
        vote_request -> Nullable<Binary>,
    }
}

table! {
    consensus_2pc_start_event (event_id) {
        event_id -> Int8,
        value -> Binary,
    }
}

table! {
    consensus_2pc_vote_event (event_id) {
        event_id -> Int8,
        vote -> Text,
    }
}

table! {
    supervisor_notification (id) {
        id -> Int8,
        circuit_id -> Text,
        service_id -> Text,
        action_id -> Int8,
        notification_type -> crate::store::scabbard_store::diesel::models::SupervisorNotificationTypeModelMapping,
        request_for_vote_value -> Nullable<Binary>,
        created_at -> Timestamp,
        executed_at -> Nullable<Timestamp>,
    }
}

joinable!(consensus_2pc_notification_action -> consensus_2pc_action (action_id));
joinable!(consensus_2pc_send_message_action -> consensus_2pc_action (action_id));
joinable!(consensus_2pc_update_context_action -> consensus_2pc_action (action_id));
joinable!(consensus_2pc_update_context_action_participant -> consensus_2pc_update_context_action (action_id));
joinable!(consensus_2pc_update_context_action_participant -> consensus_2pc_action (action_id));

joinable!(consensus_2pc_deliver_event -> consensus_2pc_event(event_id));
joinable!(consensus_2pc_start_event -> consensus_2pc_event(event_id));
joinable!(consensus_2pc_vote_event -> consensus_2pc_event(event_id));

allow_tables_to_appear_in_same_query!(
    scabbard_peer,
    scabbard_service,
    scabbard_v3_commit_history,
    scabbard_alarm,
    consensus_2pc_context,
    consensus_2pc_context_participant,
    consensus_2pc_action,
    consensus_2pc_update_context_action,
    consensus_2pc_send_message_action,
    consensus_2pc_notification_action,
    consensus_2pc_update_context_action_participant,
    consensus_2pc_event,
    consensus_2pc_deliver_event,
    consensus_2pc_start_event,
    consensus_2pc_vote_event,
);
