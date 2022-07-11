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

-- Update columns using text type to represent a boolean value to be boolean type
ALTER TABLE consensus_2pc_context DROP CONSTRAINT new_consensus_2pc_context_check1;

ALTER TABLE consensus_2pc_context ALTER COLUMN vote TYPE BOOLEAN USING
  CASE WHEN vote='FALSE' THEN FALSE
  WHEN vote='TRUE' THEN TRUE
  ELSE NULL
END;

ALTER TABLE consensus_2pc_context ADD CONSTRAINT new_consensus_2pc_context_check1
  CHECK ((vote IS NOT NULL) OR (state != 'VOTED'));

ALTER TABLE consensus_2pc_context_participant DROP CONSTRAINT new_consensus_2pc_context_participant_vote_check;

ALTER TABLE consensus_2pc_context_participant ALTER COLUMN vote TYPE BOOLEAN USING
  CASE WHEN vote='FALSE' THEN FALSE
  WHEN vote='TRUE' THEN TRUE
  ELSE NULL
END;

ALTER TABLE consensus_2pc_deliver_event DROP CONSTRAINT consensus_2pc_deliver_event_check;

ALTER TABLE consensus_2pc_deliver_event ALTER COLUMN vote_response TYPE BOOLEAN USING
  CASE WHEN vote_response='FALSE' THEN FALSE
  WHEN vote_response='TRUE' THEN TRUE
  ELSE NULL
END;

ALTER TABLE consensus_2pc_deliver_event ADD CONSTRAINT consensus_2pc_deliver_event_check
  CHECK ((vote_response IS NOT NULL) OR (message_type != 'VOTERESPONSE'));

ALTER TABLE consensus_2pc_send_message_action DROP CONSTRAINT consensus_2pc_send_message_action_check;

ALTER TABLE consensus_2pc_send_message_action ALTER COLUMN vote_response TYPE BOOLEAN USING
  CASE WHEN vote_response='FALSE' THEN FALSE
  WHEN vote_response='TRUE' THEN TRUE
  ELSE NULL
END;

ALTER TABLE consensus_2pc_send_message_action ADD CONSTRAINT consensus_2pc_send_message_action_check
  CHECK ((vote_response IS NOT NULL) OR (message_type != 'VOTERESPONSE'));

ALTER TABLE consensus_2pc_update_context_action DROP CONSTRAINT consensus_2pc_update_context_action_check1;

ALTER TABLE consensus_2pc_update_context_action ALTER COLUMN vote TYPE BOOLEAN USING
  CASE WHEN vote='FALSE' THEN FALSE
  WHEN vote='TRUE' THEN TRUE
  ELSE NULL
END;

ALTER TABLE consensus_2pc_update_context_action ADD CONSTRAINT consensus_2pc_update_context_action_check1
  CHECK ((vote IS NOT NULL) OR (state != 'VOTED'));

ALTER TABLE consensus_2pc_update_context_action_participant DROP CONSTRAINT consensus_2pc_update_context_action_participant_vote_check;

ALTER TABLE consensus_2pc_update_context_action_participant ALTER COLUMN vote TYPE BOOLEAN USING
  CASE WHEN vote='FALSE' THEN FALSE
  WHEN vote='TRUE' THEN TRUE
  ELSE NULL
END;

ALTER TABLE consensus_2pc_vote_event DROP CONSTRAINT consensus_2pc_vote_event_vote_check;

ALTER TABLE consensus_2pc_vote_event ALTER COLUMN vote TYPE BOOLEAN USING
  CASE WHEN vote='FALSE' THEN FALSE
  WHEN vote='TRUE' THEN TRUE
  ELSE NULL
END;

-- create new types with underscores in the enum variants that are multiple words
CREATE TYPE new_context_state AS ENUM ('WAITING_FOR_START', 'VOTING', 'WAITING_FOR_VOTE', 'ABORT', 'COMMIT', 'WAITING_FOR_VOTE_REQUEST', 'VOTED', 'WAITING_FOR_DECISION_ACK');
CREATE TYPE new_message_type AS ENUM ('VOTE_RESPONSE', 'DECISION_REQUEST', 'VOTE_REQUEST', 'COMMIT', 'ABORT', 'DECISION_ACK');
CREATE TYPE new_notification_type AS ENUM ('REQUEST_FOR_START', 'COORDINATOR_REQUEST_FOR_VOTE', 'PARTICIPANT_REQUEST_FOR_VOTE', 'COMMIT', 'ABORT', 'MESSAGE_DROPPED');
CREATE TYPE new_alarm_type AS ENUM ('TWO_PHASE_COMMIT');

-- create temp columns for the columns with types being updated
ALTER TABLE consensus_2pc_context ADD COLUMN temp_state new_context_state;
ALTER TABLE consensus_2pc_deliver_event ADD COLUMN temp_message_type new_message_type;
ALTER TABLE consensus_2pc_send_message_action ADD COLUMN temp_message_type new_message_type;
ALTER TABLE consensus_2pc_update_context_action ADD COLUMN temp_state new_context_state;
ALTER TABLE consensus_2pc_notification_action ADD COLUMN temp_notification_type new_notification_type;
ALTER TABLE scabbard_alarm ADD COLUMN temp_alarm_type new_alarm_type;

-- load data into the temp columns based on the existing columns
UPDATE consensus_2pc_context SET temp_state = CASE CAST(state AS TEXT)
    WHEN 'WAITINGFORSTART' THEN CAST('WAITING_FOR_VOTE_REQUEST' AS new_context_state)
    WHEN 'WAITINGFORVOTE' THEN CAST('WAITING_FOR_VOTE' AS new_context_state)
    WHEN 'WAITINGFORVOTEREQUEST' THEN CAST('WAITING_FOR_VOTE_REQUEST' AS new_context_state)
    ELSE CAST(CAST(state AS TEXT) AS new_context_state)
  END;

UPDATE consensus_2pc_deliver_event SET temp_message_type = CASE CAST(message_type AS TEXT)
    WHEN 'VOTERESPONSE' THEN CAST('VOTE_RESPONSE' AS new_message_type)
    WHEN 'DECISIONREQUEST' THEN CAST('DECISION_REQUEST' AS new_message_type)
    WHEN 'VOTEREQUEST' THEN CAST('VOTE_REQUEST' AS new_message_type)
    ELSE CAST(CAST(message_type AS TEXT) AS new_message_type)
  END;

UPDATE consensus_2pc_send_message_action SET temp_message_type = CASE CAST(message_type AS TEXT)
    WHEN 'VOTERESPONSE' THEN CAST('VOTE_RESPONSE' AS new_message_type)
    WHEN 'DECISIONREQUEST' THEN CAST('DECISION_REQUEST' AS new_message_type)
    WHEN 'VOTEREQUEST' THEN CAST('VOTE_REQUEST' AS new_message_type)
    ELSE CAST(CAST(message_type AS TEXT) AS new_message_type)
  END;

UPDATE consensus_2pc_update_context_action SET temp_state = CASE CAST(state AS TEXT)
    WHEN 'WAITINGFORSTART' THEN CAST('WAITING_FOR_VOTE_REQUEST' AS new_context_state)
    WHEN 'WAITINGFORVOTE' THEN CAST('WAITING_FOR_VOTE' AS new_context_state)
    WHEN 'WAITINGFORVOTEREQUEST' THEN CAST('WAITING_FOR_VOTE_REQUEST' AS new_context_state)
    ELSE CAST(CAST(state AS TEXT) AS new_context_state)
  END;

UPDATE consensus_2pc_notification_action SET temp_notification_type = CASE CAST(notification_type AS TEXT)
    WHEN 'REQUESTFORSTART' THEN CAST('REQUEST_FOR_START' AS new_notification_type)
    WHEN 'COORDINATORREQUESTFORVOTE' THEN CAST('COORDINATOR_REQUEST_FOR_VOTE' AS new_notification_type)
    WHEN 'PARTICIPANTREQUESTFORVOTE' THEN CAST('PARTICIPANT_REQUEST_FOR_VOTE' AS new_notification_type)
    WHEN 'MESSAGEDROPPED' THEN CAST('MESSAGE_DROPPED' AS new_notification_type)
    ELSE CAST(CAST(notification_type AS TEXT) AS new_notification_type)
  END;

UPDATE scabbard_alarm SET temp_alarm_type = CASE CAST(alarm_type AS TEXT)
    WHEN 'TWOPHASECOMMIT' THEN CAST('TWO_PHASE_COMMIT' AS new_alarm_type)
  END;

-- drop the old columns
ALTER TABLE consensus_2pc_context DROP COLUMN state;
ALTER TABLE consensus_2pc_deliver_event DROP COLUMN message_type;
ALTER TABLE consensus_2pc_send_message_action DROP COLUMN message_type;
ALTER TABLE consensus_2pc_update_context_action DROP COLUMN state;
ALTER TABLE consensus_2pc_notification_action DROP COLUMN notification_type;
ALTER TABLE scabbard_alarm DROP COLUMN alarm_type;

-- rename the temp columns
ALTER TABLE consensus_2pc_context RENAME COLUMN temp_state TO state;
ALTER TABLE consensus_2pc_deliver_event RENAME COLUMN temp_message_type TO message_type;
ALTER TABLE consensus_2pc_update_context_action RENAME COLUMN temp_state TO state;
ALTER TABLE consensus_2pc_send_message_action RENAME COLUMN temp_message_type TO message_type;
ALTER TABLE consensus_2pc_notification_action RENAME COLUMN temp_notification_type TO notification_type;
ALTER TABLE scabbard_alarm RENAME COLUMN temp_alarm_type TO alarm_type;

-- drop the old unused types
DROP TYPE IF EXISTS context_state;
DROP TYPE IF EXISTS message_type;
DROP TYPE IF EXISTS notification_type;
DROP TYPE IF EXISTS deliver_event_message_type;
DROP TYPE IF EXISTS alarm_type;

-- rename the new types
ALTER TYPE new_context_state RENAME TO context_state;
ALTER TYPE new_message_type RENAME TO message_type;
ALTER TYPE new_notification_type RENAME TO notification_type;
ALTER TYPE new_alarm_type RENAME TO alarm_type;
