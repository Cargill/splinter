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

CREATE TYPE supervisor_notification_type AS ENUM ( 'ABORT',
      'COMMIT',
      'REQUEST_FOR_START',
      'COORDINATOR_REQUEST_FOR_VOTE',
      'PARTICIPANT_REQUEST_FOR_VOTE');

CREATE TABLE IF NOT EXISTS supervisor_notification (
  id                            BIGSERIAL PRIMARY KEY,
  circuit_id                    TEXT NOT NULL,
  service_id                    TEXT NOT NULL,
  action_id                     BIGINT NOT NULL,
  notification_type             supervisor_notification_type NOT NULL,
  request_for_vote_value        BYTEA,
  created_at                    TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
  executed_at                   TIMESTAMP,

  FOREIGN KEY (circuit_id, service_id) REFERENCES scabbard_service(circuit_id, service_id) ON DELETE CASCADE,
  FOREIGN KEY (action_id) REFERENCES consensus_2pc_action(id) ON DELETE CASCADE
);
