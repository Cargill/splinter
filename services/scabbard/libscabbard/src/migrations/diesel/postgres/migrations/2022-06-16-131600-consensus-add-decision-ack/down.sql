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

ALTER TABLE consensus_2pc_context_participant DROP COLUMN decision_ack;

ALTER TABLE consensus_2pc_update_context_action_participant DROP COLUMN decision_ack;


CREATE TYPE new_message_type AS ENUM ('VOTERESPONSE', 'DECISIONREQUEST', 'VOTEREQUEST', 'COMMIT', 'ABORT');
CREATE TYPE new_deliver_event_message_type AS ENUM ('VOTERESPONSE', 'DECISIONREQUEST', 'VOTEREQUEST', 'COMMIT', 'ABORT');

DELETE FROM consensus_2pc_send_message_action WHERE message_type = 'DECISION_ACK';
DELETE FROM consensus_2pc_deliver_event WHERE message_type = 'DECISION_ACK';

ALTER TABLE consensus_2pc_send_message_action
  ALTER COLUMN message_type TYPE new_message_type NOT NULL;
ALTER TABLE consensus_2pc_deliver_event
  ALTER COLUMN message_type TYPE new_deliver_event_message_type NOT NULL;

DROP TYPE message_type;
DROP TYPE deliver_event_message_type;

ALTER TYPE new_message_type RENAME TO message_type;
ALTER TYPE new_deliver_event_message_type RENAME TO deliver_event_message_type;


ALTER TABLE consensus_2pc_context DROP COLUMN ack_timeout_start;

CREATE TYPE new_context_state AS ENUM ('WAITINGFORSTART', 'VOTING', 'WAITINGFORVOTE', 'ABORT', 'COMMIT', 'WAITINGFORVOTEREQUEST', 'VOTED');

DELETE FROM consensus_2pc_context WHERE state = 'WAITING_FOR_DECISION_ACK';

ALTER TABLE consensus_2pc_context
  ALTER COLUMN state TYPE new_context_state NOT NULL;

DROP TYPE context_state;

ALTER TYPE new_context_state RENAME TO context_state;
