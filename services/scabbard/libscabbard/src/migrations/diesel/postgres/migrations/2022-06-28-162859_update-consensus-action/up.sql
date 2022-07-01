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

CREATE TYPE action_type AS ENUM ('UPDATE_CONTEXT', 'SEND_MESSAGE', 'NOTIFICATION');

ALTER TABLE consensus_2pc_action ADD COLUMN action_type action_type;

UPDATE consensus_2pc_action SET action_type='NOTIFICATION' 
FROM consensus_2pc_notification_action WHERE 
consensus_2pc_action.id=consensus_2pc_notification_action.action_id;

UPDATE consensus_2pc_action SET action_type='UPDATE_CONTEXT' 
FROM consensus_2pc_update_context_action WHERE 
consensus_2pc_action.id=consensus_2pc_update_context_action.action_id;

UPDATE consensus_2pc_action SET action_type='SEND_MESSAGE' 
FROM consensus_2pc_send_message_action WHERE 
consensus_2pc_action.id=consensus_2pc_send_message_action.action_id;

ALTER TABLE consensus_2pc_action ALTER COLUMN action_type SET NOT NULL;

ALTER TABLE consensus_2pc_action ADD COLUMN event_id BIGINT;

UPDATE consensus_2pc_action SET event_id = (
    SELECT consensus_2pc_event.id as event_id FROM consensus_2pc_event
    INNER JOIN consensus_2pc_action 
    	ON consensus_2pc_action.circuit_id = consensus_2pc_event.circuit_id AND
    	consensus_2pc_action.service_id = consensus_2pc_event.service_id
    ORDER BY consensus_2pc_event.id DESC LIMIT 1
);

ALTER TABLE consensus_2pc_action ALTER COLUMN event_id SET NOT NULL;
ALTER TABLE consensus_2pc_action ADD CONSTRAINT event_id
FOREIGN KEY (event_id) REFERENCES consensus_2pc_event (id) ON DELETE CASCADE;

ALTER TABLE consensus_2pc_action 
ALTER COLUMN executed_at type TIMESTAMP USING to_timestamp(executed_at);
