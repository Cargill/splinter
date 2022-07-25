-- Copyright 2018-2022 Cargill Incorporated
--
-- Licensed under the Apache License, Version 2.0 (the "License");
-- you may not use this file except in compliance with the Licens
-- You may obtain a copy of the License at
--
--     http://www.apachorg/licenses/LICENSE-2.0
--
-- Unless required by applicable law or agreed to in writing, software
-- distributed under the License is distributed on an "AS IS" BASIS,
-- WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
-- See the License for the specific language governing permissions and
-- limitations under the Licens
-- -----------------------------------------------------------------------------

CREATE VIEW consensus_2pc_events_and_contexts_all AS
SELECT id AS e_id,
       circuit_id,
       service_id,
       event_type,
       consensus_2pc_deliver_event.epoch AS d_epoch,
       consensus_2pc_deliver_event.receiver_service_id AS d_receiver_service_id,
       consensus_2pc_deliver_event.message_type AS d_message_type,
       consensus_2pc_deliver_event.vote_response AS d_vote_response,
       consensus_2pc_deliver_event.vote_request AS d_vote_request,
       consensus_2pc_start_event.value AS s_value,
       consensus_2pc_vote_event.vote AS v_vote,
       created_at AS e_created_at,
       executed_at AS e_executed_at,
       executed_epoch AS e_executed_epoch,
       consensus_2pc_update_context_action.coordinator AS ctx_coordinator,
       consensus_2pc_update_context_action.epoch AS ctx_epoch,
       consensus_2pc_update_context_action.last_commit_epoch AS ctx_last_commit_epoch,
       consensus_2pc_update_context_action.state AS ctx_state,
       consensus_2pc_update_context_action.vote_timeout_start AS ctx_vote_timeout_start,
       consensus_2pc_update_context_action.vote AS ctx_vote,
       consensus_2pc_update_context_action.decision_timeout_start AS ctx_decision_timeout_start,
       consensus_2pc_update_context_action.action_alarm AS ctx_action_alarm,
       consensus_2pc_update_context_action.ack_timeout_start AS ctx_ack_timeout_start
FROM consensus_2pc_event
LEFT JOIN consensus_2pc_deliver_event
    ON consensus_2pc_event.id=consensus_2pc_deliver_event.event_id
LEFT JOIN consensus_2pc_start_event
    ON consensus_2pc_event.id=consensus_2pc_start_event.event_id
LEFT JOIN consensus_2pc_vote_event
    ON consensus_2pc_event.id=consensus_2pc_vote_event.event_id
LEFT JOIN consensus_2pc_update_context_action 
    ON consensus_2pc_event.update_context_action_id = consensus_2pc_update_context_action.action_id
ORDER BY id;
