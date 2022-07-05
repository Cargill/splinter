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

-- consensus_2pc_context
ALTER TABLE consensus_2pc_context DROP CONSTRAINT new_consensus_2pc_context_check1;

ALTER TABLE consensus_2pc_context ALTER COLUMN vote TYPE BOOLEAN USING
  CASE WHEN vote='FALSE' THEN FALSE
  WHEN vote='TRUE' THEN TRUE
  ELSE NULL
END;

ALTER TABLE consensus_2pc_context ADD CONSTRAINT new_consensus_2pc_context_check1
  CHECK ((vote IS NOT NULL) OR (state != 'VOTED'));

-- consensus_2pc_context_participant
ALTER TABLE consensus_2pc_context_participant DROP CONSTRAINT new_consensus_2pc_context_participant_vote_check;

ALTER TABLE consensus_2pc_context_participant ALTER COLUMN vote TYPE BOOLEAN USING
  CASE WHEN vote='FALSE' THEN FALSE
  WHEN vote='TRUE' THEN TRUE
  ELSE NULL
END;

-- consensus_2pc_deliver_event
ALTER TABLE consensus_2pc_deliver_event DROP CONSTRAINT consensus_2pc_deliver_event_check;

ALTER TABLE consensus_2pc_deliver_event ALTER COLUMN vote_response TYPE BOOLEAN USING
  CASE WHEN vote_response='FALSE' THEN FALSE
  WHEN vote_response='TRUE' THEN TRUE
  ELSE NULL
END;

ALTER TABLE consensus_2pc_deliver_event ADD CONSTRAINT consensus_2pc_deliver_event_check
  CHECK ((vote_response IS NOT NULL) OR (message_type != 'VOTERESPONSE'));

-- consensus_2pc_send_message_action
ALTER TABLE consensus_2pc_send_message_action DROP CONSTRAINT consensus_2pc_send_message_action_check;

ALTER TABLE consensus_2pc_send_message_action ALTER COLUMN vote_response TYPE BOOLEAN USING
  CASE WHEN vote_response='FALSE' THEN FALSE
  WHEN vote_response='TRUE' THEN TRUE
  ELSE NULL
END;

ALTER TABLE consensus_2pc_send_message_action ADD CONSTRAINT consensus_2pc_send_message_action_check
  CHECK ((vote_response IS NOT NULL) OR (message_type != 'VOTERESPONSE'));

-- consensus_2pc_update_context_action
ALTER TABLE consensus_2pc_update_context_action DROP CONSTRAINT consensus_2pc_update_context_action_check1;

ALTER TABLE consensus_2pc_update_context_action ALTER COLUMN vote TYPE BOOLEAN USING
  CASE WHEN vote='FALSE' THEN FALSE
  WHEN vote='TRUE' THEN TRUE
  ELSE NULL
END;

ALTER TABLE consensus_2pc_update_context_action ADD CONSTRAINT consensus_2pc_update_context_action_check1
  CHECK ((vote IS NOT NULL) OR (state != 'VOTED'));

-- consensus_2pc_update_context_action_participant
ALTER TABLE consensus_2pc_update_context_action_participant DROP CONSTRAINT consensus_2pc_update_context_action_participant_vote_check;

ALTER TABLE consensus_2pc_update_context_action_participant ALTER COLUMN vote TYPE BOOLEAN USING
  CASE WHEN vote='FALSE' THEN FALSE
  WHEN vote='TRUE' THEN TRUE
  ELSE NULL
END;

-- consensus_2pc_vote_event
ALTER TABLE consensus_2pc_vote_event DROP CONSTRAINT consensus_2pc_vote_event_vote_check;

ALTER TABLE consensus_2pc_vote_event ALTER COLUMN vote TYPE BOOLEAN USING
  CASE WHEN vote='FALSE' THEN FALSE
  WHEN vote='TRUE' THEN TRUE
  ELSE NULL
END;
