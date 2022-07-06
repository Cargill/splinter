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

ALTER TABLE consensus_2pc_context ALTER COLUMN vote TYPE TEXT USING
  CASE WHEN vote=FALSE THEN 'FALSE'
  WHEN vote=TRUE THEN 'TRUE'
  ELSE NULL
END;

ALTER TABLE consensus_2pc_context ADD CONSTRAINT new_consensus_2pc_context_check1
  CHECK ((vote IN ('TRUE' , 'FALSE')) OR ( state != 'VOTED'));

-- consensus_2pc_context_participant
ALTER TABLE consensus_2pc_context_participant ALTER COLUMN vote TYPE TEXT USING
  CASE WHEN vote=FALSE THEN 'FALSE'
  WHEN vote=TRUE THEN 'TRUE'
  ELSE NULL
END;

ALTER TABLE consensus_2pc_context ADD CONSTRAINT new_consensus_2pc_context_participant_vote_check
  CHECK (vote IN ('TRUE' , 'FALSE') OR vote IS NULL);

-- consensus_2pc_deliver_event
ALTER TABLE consensus_2pc_deliver_event DROP CONSTRAINT consensus_2pc_deliver_event_check;

ALTER TABLE consensus_2pc_deliver_event ALTER COLUMN vote_response TYPE TEXT USING
  CASE WHEN vote_response=FALSE THEN 'FALSE'
  WHEN vote_response=TRUE THEN 'TRUE'
  ELSE NULL
END;

ALTER TABLE consensus_2pc_deliver_event ADD CONSTRAINT consensus_2pc_deliver_event_check
  CHECK ((vote_response IN ('TRUE', 'FALSE')) OR (message_type != 'VOTERESPONSE'));

-- consensus_2pc_send_message_action
ALTER TABLE consensus_2pc_send_message_action DROP CONSTRAINT consensus_2pc_send_message_action_check;

ALTER TABLE consensus_2pc_send_message_action ALTER COLUMN vote_response TYPE TEXT USING
  CASE WHEN vote_response=FALSE THEN 'FALSE'
  WHEN vote_response=TRUE THEN 'TRUE'
  ELSE NULL
END;

ALTER TABLE consensus_2pc_send_message_action ADD CONSTRAINT consensus_2pc_send_message_action_check
  CHECK ((vote_response IN ('TRUE', 'FALSE')) OR (message_type != 'VOTERESPONSE'));

-- consensus_2pc_update_context_action
ALTER TABLE consensus_2pc_update_context_action DROP CONSTRAINT consensus_2pc_update_context_action_check1;

ALTER TABLE consensus_2pc_update_context_action ALTER COLUMN vote TYPE TEXT USING
  CASE WHEN vote=FALSE THEN 'FALSE'
  WHEN vote=TRUE THEN 'TRUE'
  ELSE NULL
END;

ALTER TABLE consensus_2pc_update_context_action ADD CONSTRAINT consensus_2pc_update_context_action_check1
  CHECK ((vote IN ('TRUE' , 'FALSE')) OR ( state != 'VOTED'));

-- consensus_2pc_update_context_action_participant
ALTER TABLE consensus_2pc_update_context_action_participant ALTER COLUMN vote TYPE TEXT USING
  CASE WHEN vote=FALSE THEN 'FALSE'
  WHEN vote=TRUE THEN 'TRUE'
  ELSE NULL
END;

ALTER TABLE consensus_2pc_update_context_action_participant ADD CONSTRAINT consensus_2pc_update_context_action_participant_vote_check
  CHECK (vote IN ('TRUE' , 'FALSE') OR vote IS NULL);

-- consensus_2pc_vote_event
ALTER TABLE consensus_2pc_vote_event ALTER COLUMN vote TYPE TEXT USING
  CASE WHEN vote=FALSE THEN 'FALSE'
  WHEN vote=TRUE THEN 'TRUE'
  ELSE NULL
END;

ALTER TABLE consensus_2pc_vote_event ADD CONSTRAINT consensus_2pc_vote_event_vote_check
  CHECK (vote IN ('TRUE' , 'FALSE'));

-- recreate new old types without underscores in the enum variants that are multiple words
CREATE TYPE new_context_state AS ENUM ('WAITINGFORSTART', 'VOTING', 'WAITINGFORVOTE', 'ABORT', 'COMMIT', 'WAITINGFORVOTEREQUEST', 'VOTED', 'WAITING_FOR_DECISION_ACK');
CREATE TYPE new_message_type AS ENUM ('VOTERESPONSE', 'DECISIONREQUEST', 'VOTEREQUEST', 'COMMIT', 'ABORT', 'DECISION_ACK');
CREATE TYPE new_deliver_event_message_type AS ENUM ('VOTERESPONSE', 'DECISIONREQUEST', 'VOTEREQUEST', 'COMMIT', 'ABORT', 'DECISION_ACK');
CREATE TYPE new_notification_type AS ENUM ('REQUESTFORSTART', 'COORDINATORREQUESTFORVOTE', 'PARTICIPANTREQUESTFORVOTE', 'COMMIT', 'ABORT', 'MESSAGEDROPPED');
CREATE TYPE new_alarm_type AS ENUM ('TWOPHASECOMMIT');

-- create temp columns for the columns that were updated
ALTER TABLE consensus_2pc_context ADD COLUMN temp_state new_context_state;
ALTER TABLE consensus_2pc_deliver_event ADD COLUMN temp_message_type new_deliver_event_message_type;
ALTER TABLE consensus_2pc_send_message_action ADD COLUMN temp_message_type new_message_type;
ALTER TABLE consensus_2pc_update_context_action ADD COLUMN temp_state new_context_state;
ALTER TABLE consensus_2pc_notification_action ADD COLUMN temp_notification_type new_notification_type;
ALTER TABLE scabbard_alarm ADD COLUMN temp_alarm_type new_alarm_type;

-- load data into the temp columns based on the existing columns
UPDATE consensus_2pc_context SET temp_state = CASE CAST(state AS TEXT)
    WHEN 'WAITING_FOR_START' THEN CAST('WAITINGFORSTART' AS new_context_state)
    WHEN 'WAITING_FOR_VOTE' THEN CAST('WAITINGFORVOTE' AS new_context_state)
    WHEN 'WAITING_FOR_VOTE_REQUEST' THEN CAST('WAITINGFORVOTEREQUEST' AS new_context_state)
    ELSE CAST(CAST(state AS TEXT) AS new_context_state)
  END;

UPDATE consensus_2pc_deliver_event SET temp_message_type = CASE CAST(message_type AS TEXT)
    WHEN 'VOTE_RESPONSE' THEN CAST('VOTERESPONSE' AS new_deliver_event_message_type)
    WHEN 'DECISION_REQUEST' THEN CAST('DECISIONREQUEST' AS new_deliver_event_message_type)
    WHEN 'VOTE_REQUEST' THEN CAST('VOTEREQUEST' AS new_deliver_event_message_type)
    ELSE CAST(CAST(message_type AS TEXT) AS new_deliver_event_message_type)
  END;

UPDATE consensus_2pc_send_message_action SET temp_message_type = CASE CAST(message_type AS TEXT)
    WHEN 'VOTE_RESPONSE' THEN CAST('VOTERESPONSE'
    WHEN 'DECISION_REQUEST' THEN CAST('DECISIONREQUEST'
    WHEN 'VOTE_REQUEST' THEN CAST('VOTEREQUEST'
    ELSE CAST(CAST(message_type AS TEXT) AS new_message_type)
  END;

UPDATE consensus_2pc_update_context_action SET temp_state = CASE CAST(state AS TEXT)
    WHEN 'WAITING_FOR_START' THEN CAST('WAITINGFORSTART' AS new_context_state)
    WHEN 'WAITING_FOR_VOTE' THEN CAST('WAITINGFORVOTE' AS new_context_state)
    WHEN 'WAITING_FOR_VOTE_REQUEST' THEN CAST('WAITINGFORVOTEREQUEST' AS new_context_state)
    ELSE CAST(CAST(state AS TEXT) AS new_context_state)
  END;

UPDATE consensus_2pc_notification_action SET temp_notification_type = CASE CAST(notification_type AS TEXT)
    WHEN 'REQUEST_FOR_START' THEN CAST('REQUESTFORSTART' AS new_notification_type)
    WHEN 'COORDINATOR_REQUEST_FOR_VOTE' THEN CAST('COORDINATORREQUESTFORVOTE' AS new_notification_type)
    WHEN 'PARTICIPANT_REQUEST_FOR_VOTE' THEN CAST('PARTICIPANTREQUESTFORVOTE' AS new_notification_type)
    WHEN 'MESSAGE_DROPPED' THEN CAST('MESSAGEDROPPED' AS new_notification_type)
    ELSE CAST(CAST(notification_type AS TEXT) AS new_notification_type)
  END;

UPDATE scabbard_alarm SET temp_alarm_type = CASE CAST(alarm_type AS TEXT)
    WHEN 'TWO_PHASE_COMMIT' THEN CAST('TWOPHASECOMMIT' AS new_alarm_type)
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

-- drop the unused types
DROP TYPE IF EXISTS context_state;
DROP TYPE IF EXISTS message_type;
DROP TYPE IF EXISTS notification_type;
DROP TYPE IF EXISTS alarm_type;

-- rename types
ALTER TYPE new_context_state RENAME TO context_state;
ALTER TYPE new_message_type RENAME TO message_type;
ALTER TYPE new_deliver_event_message_type RENAME TO deliver_event_message_type;
ALTER TYPE new_notification_type RENAME TO notification_type;
ALTER TYPE new_alarm_type RENAME TO alarm_type;
