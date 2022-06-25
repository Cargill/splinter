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

ALTER TABLE consensus_2pc_update_context_action DROP COLUMN ack_timeout_start;

ALTER TABLE consensus_2pc_context DROP CONSTRAINT ack_timeout_start_check;

ALTER TABLE consensus_2pc_context ADD CONSTRAINT ack_timeout_start
  CHECK ( (ack_timeout_start IS NOT NULL) OR (state != 'WAITING_FOR_DECISION_ACK') );

ALTER TABLE scabbard_v3_commit_history
ALTER COLUMN epoch TYPE INTEGER;

ALTER TABLE consensus_2pc_deliver_event
ALTER COLUMN event_id TYPE INTEGER;

ALTER TABLE consensus_2pc_start_event
ALTER COLUMN event_id TYPE INTEGER;

ALTER TABLE consensus_2pc_vote_event
ALTER COLUMN event_id TYPE INTEGER;

ALTER TABLE consensus_2pc_update_context_action
ALTER COLUMN action_id TYPE INTEGER;

ALTER TABLE consensus_2pc_send_message_action
ALTER COLUMN action_id TYPE INTEGER;

ALTER TABLE consensus_2pc_notification_action
ALTER COLUMN action_id TYPE INTEGER;

ALTER TABLE consensus_2pc_update_context_action_participant
ALTER COLUMN action_id TYPE INTEGER;
