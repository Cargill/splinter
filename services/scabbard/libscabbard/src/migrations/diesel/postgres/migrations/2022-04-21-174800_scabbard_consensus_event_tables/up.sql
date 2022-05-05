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

CREATE TYPE event_type AS ENUM ('ALARM', 'DELIVER', 'START', 'VOTE');
CREATE TYPE deliver_event_message_type AS ENUM ('VOTERESPONSE', 'DECISIONREQUEST', 'VOTEREQUEST', 'COMMIT', 'ABORT');

CREATE TABLE IF NOT EXISTS consensus_2pc_event (
    id                        BIGSERIAL PRIMARY KEY,
    service_id                TEXT NOT NULL,
    epoch                     BIGINT NOT NULL,
    created_at                TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    executed_at               BIGINT,
    position                  INTEGER NOT NULL,
    event_type                event_type NOT NULL
);

CREATE TABLE IF NOT EXISTS consensus_2pc_deliver_event (
    event_id                  INTEGER PRIMARY KEY,
    service_id                TEXT NOT NULL,
    epoch                     BIGINT NOT NULL,
    receiver_service_id       TEXT NOT NULL,
    message_type              deliver_event_message_type NOT NULL,
    vote_response             TEXT
    CHECK ( (vote_response IN ('TRUE', 'FALSE')) OR (message_type != 'VOTERESPONSE') ),
    vote_request              BYTEA
    CHECK ( (vote_request IS NOT NULL) OR (message_type != 'VOTEREQUEST') ),
    FOREIGN KEY (event_id) REFERENCES consensus_2pc_event(id) ON DELETE CASCADE,
    FOREIGN KEY (service_id, epoch) REFERENCES consensus_2pc_context(service_id, epoch) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS consensus_2pc_start_event (
    event_id                  INTEGER PRIMARY KEY,
    service_id                TEXT NOT NULL,
    epoch                     BIGINT NOT NULL,
    value                     BYTEA,
    FOREIGN KEY (event_id) REFERENCES consensus_2pc_event(id) ON DELETE CASCADE,
    FOREIGN KEY (service_id, epoch) REFERENCES consensus_2pc_context(service_id, epoch) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS consensus_2pc_vote_event (
    event_id                  INTEGER PRIMARY KEY,
    service_id                TEXT NOT NULL,
    epoch                     BIGINT NOT NULL,
    vote                      TEXT
    CHECK ( vote IN ('TRUE' , 'FALSE') ),
    FOREIGN KEY (event_id) REFERENCES consensus_2pc_event(id) ON DELETE CASCADE,
    FOREIGN KEY (service_id, epoch) REFERENCES consensus_2pc_context(service_id, epoch) ON DELETE CASCADE
);
