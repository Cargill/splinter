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

CREATE TABLE IF NOT EXISTS scabbard_service (
    service_id       TEXT PRIMARY KEY NOT NULL,
    status           TEXT NOT NULL
    CHECK ( status IN ('PREPARED', 'FINALIZED', 'RETIRED') )
);

CREATE TABLE IF NOT EXISTS scabbard_peer (
    service_id       TEXT NOT NULL,
    peer_service_id  TEXT,
    PRIMARY KEY(service_id, peer_service_id),
    FOREIGN KEY(service_id) REFERENCES scabbard_service(service_id)
);

CREATE TABLE IF NOT EXISTS scabbard_v3_commit_history (
    service_id TEXT NOT NULL,
    id      INTEGER NOT NULL,
    value      TEXT NOT NULL,
    decision   TEXT,
    CHECK ( decision IN ('COMMIT', 'ABORT') ),
    PRIMARY KEY (service_id, id)
);
