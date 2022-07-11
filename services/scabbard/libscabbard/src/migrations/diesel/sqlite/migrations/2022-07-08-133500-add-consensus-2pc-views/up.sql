-- Copyright 2018-2022 Cargill Incorporated
--
-- Licensed under the Apache License, Version 2.0 (the "License");
-- you may not use this file except in compliance with the License.
-- You may obtain a copy of the License at
--
--     http://www.apache.org/licenses/LICENSE-2.0
--
-- Unless required by applicable law or agreed to in writing, software
-- distributed under the License is distributed on an "AS IS" BASIS,
-- WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
-- See the License for the specific language governing permissions and
-- limitations under the License.
-- -----------------------------------------------------------------------------

-- ALL ACTIONS
CREATE VIEW IF NOT EXISTS consensus_2pc_actions_all AS
SELECT id,
       event_id,
       action_type,
       circuit_id,
       service_id,
       consensus_2pc_notification_action.notification_type as n_notification_type,
       consensus_2pc_notification_action.dropped_message as n_dropped_message,
       consensus_2pc_notification_action.request_for_vote_value as n_request_for_vote_value,
       consensus_2pc_send_message_action.epoch as s_epoch,
       consensus_2pc_send_message_action.receiver_service_id as s_receiver_service_id,
       consensus_2pc_send_message_action.message_type as s_message_type,
       consensus_2pc_send_message_action.vote_response as s_vote_response,
       consensus_2pc_send_message_action.vote_request as s_vote_request,
       consensus_2pc_update_context_action.coordinator as uc_coordinator,
       consensus_2pc_update_context_action.epoch as uc_epoch,
       consensus_2pc_update_context_action.last_commit_epoch as uc_last_commit_epoch,
       consensus_2pc_update_context_action.state as uc_state,
       consensus_2pc_update_context_action.vote_timeout_start as uc_vote_timeout_start,
       consensus_2pc_update_context_action.vote as uc_vote,
       consensus_2pc_update_context_action.decision_timeout_start as uc_decision_timeout_start,
       consensus_2pc_update_context_action.action_alarm as uc_action_alarm,
       consensus_2pc_update_context_action.ack_timeout_start as uc_ack_timeout_start,
       consensus_2pc_update_context_action_participant.process as ucp_process,
       consensus_2pc_update_context_action_participant.vote as ucp_vote,
       consensus_2pc_update_context_action_participant.decision_ack as ucp_decision_ack,
       created_at,
       executed_at
FROM consensus_2pc_action
LEFT JOIN consensus_2pc_notification_action ON consensus_2pc_action.id=consensus_2pc_notification_action.action_id
LEFT JOIN consensus_2pc_send_message_action ON consensus_2pc_action.id=consensus_2pc_send_message_action.action_id
LEFT JOIN consensus_2pc_update_context_action ON consensus_2pc_action.id=consensus_2pc_update_context_action.action_id
LEFT JOIN consensus_2pc_update_context_action_participant ON consensus_2pc_action.id=consensus_2pc_update_context_action_participant.action_id
ORDER BY id;

-- ALL EVENTS
CREATE VIEW IF NOT EXISTS consensus_2pc_events_all AS
SELECT id,
       circuit_id,
       service_id,
       event_type,
       consensus_2pc_deliver_event.epoch as d_epoch,
       consensus_2pc_deliver_event.receiver_service_id as d_receiver_service_id,
       consensus_2pc_deliver_event.message_type as d_message_type,
       consensus_2pc_deliver_event.vote_response as d_vote_response,
       consensus_2pc_deliver_event.vote_request as d_vote_request,
       consensus_2pc_start_event.value as s_value,
       consensus_2pc_vote_event.vote as v_vote,
       created_at,
       executed_at,
       executed_epoch
FROM consensus_2pc_event
LEFT JOIN consensus_2pc_deliver_event ON consensus_2pc_event.id=consensus_2pc_deliver_event.event_id
LEFT JOIN consensus_2pc_start_event ON
consensus_2pc_event.id=consensus_2pc_start_event.event_id
LEFT JOIN consensus_2pc_vote_event ON
consensus_2pc_event.id=consensus_2pc_vote_event.event_id
ORDER BY id;

-- ALL ACTIONS AND EVENTS
CREATE VIEW IF NOT EXISTS consensus_2pc_actions_and_events_all AS 
SELECT id AS a_id,
       event_id AS a_event_id,
       action_type AS a_event_type,
       consensus_2pc_notification_action.notification_type as n_notification_type,
       consensus_2pc_notification_action.dropped_message as n_dropped_message,
       consensus_2pc_notification_action.request_for_vote_value as n_request_for_vote_value,
       consensus_2pc_send_message_action.epoch as s_epoch,
       consensus_2pc_send_message_action.receiver_service_id as s_receiver_service_id,
       consensus_2pc_send_message_action.message_type as s_message_type,
       consensus_2pc_send_message_action.vote_response as s_vote_response,
       consensus_2pc_send_message_action.vote_request as s_vote_request,
       consensus_2pc_update_context_action.coordinator as uc_coordinator,
       consensus_2pc_update_context_action.epoch as uc_epoch,
       consensus_2pc_update_context_action.last_commit_epoch as uc_last_commit_epoch,
       consensus_2pc_update_context_action.state as uc_state,
       consensus_2pc_update_context_action.vote_timeout_start as uc_vote_timeout_start,
       consensus_2pc_update_context_action.vote as uc_vote,
       consensus_2pc_update_context_action.decision_timeout_start as uc_decision_timeout_start,
       consensus_2pc_update_context_action.action_alarm as uc_action_alarm,
       consensus_2pc_update_context_action.ack_timeout_start as uc_ack_timeout_start,
       consensus_2pc_update_context_action_participant.process as ucp_process,
       consensus_2pc_update_context_action_participant.vote as ucp_vote,
       consensus_2pc_update_context_action_participant.decision_ack as ucp_decision_ack,
       circuit_id,
       service_id,
       created_at,
       executed_at,
       '' as e_id,
       '' as e_event_type,
       '' as d_epoch,
       '' as d_receiver_service_id,
       '' as d_message_type,
       '' as d_vote_response,
       '' as d_vote_request,
       '' as s_value,
       '' as v_vote,
       '' as e_executed_epoch
FROM consensus_2pc_action
LEFT JOIN consensus_2pc_notification_action ON consensus_2pc_action.id=consensus_2pc_notification_action.action_id
LEFT JOIN consensus_2pc_send_message_action ON consensus_2pc_action.id=consensus_2pc_send_message_action.action_id
LEFT JOIN consensus_2pc_update_context_action ON consensus_2pc_action.id=consensus_2pc_update_context_action.action_id
LEFT JOIN consensus_2pc_update_context_action_participant ON consensus_2pc_action.id=consensus_2pc_update_context_action_participant.action_id
UNION
SELECT '' AS a_id,
       '' AS a_event_id,
       '' AS a_event_type,
       '' as n_notification_type,
       '' as n_dropped_message,
       '' as n_request_for_vote_value,
       '' as s_epoch,
       '' as s_receiver_service_id,
       '' as s_message_type,
       '' as s_vote_response,
       '' as s_vote_request,
       '' as uc_coordinator,
       ''  as uc_epoch,
       '' as uc_last_commit_epoch,
       '' as uc_state,
       '' as uc_vote_timeout_start,
       '' as uc_vote,
       '' as uc_decision_timeout_start,
       '' as uc_action_alarm,
       '' as uc_ack_timeout_start,
       '' as ucp_process,
       '' as ucp_vote,
       '' as ucp_decision_ack,
       circuit_id,
       service_id,
       created_at,
       executed_at,
       id as e_id,
       event_type as e_event_type,
       consensus_2pc_deliver_event.epoch as d_epoch,
       consensus_2pc_deliver_event.receiver_service_id as d_receiver_service_id,
       consensus_2pc_deliver_event.message_type as d_message_type,
       consensus_2pc_deliver_event.vote_response as d_vote_response,
       consensus_2pc_deliver_event.vote_request as d_vote_request,
       consensus_2pc_start_event.value as s_value,
       consensus_2pc_vote_event.vote as v_vote,
       executed_epoch as e_executed_epoch
FROM consensus_2pc_event
LEFT JOIN consensus_2pc_deliver_event ON consensus_2pc_event.id=consensus_2pc_deliver_event.event_id
LEFT JOIN consensus_2pc_start_event ON
consensus_2pc_event.id=consensus_2pc_start_event.event_id
LEFT JOIN consensus_2pc_vote_event ON
consensus_2pc_event.id=consensus_2pc_vote_event.event_id
ORDER BY executed_at;
