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

CREATE TYPE command_type AS ENUM ('PREPARE', 'FINALIZE', 'RETIRE', 'PURGE');
CREATE TYPE status_type AS ENUM ('NEW', 'COMPLETE');

CREATE TABLE IF NOT EXISTS service_lifecycle_status (
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    service_type              TEXT NOT NULL,
    status                    status_type NOT NULL,
    command                   command_type NOT NULL,
    PRIMARY KEY (circuit_id, service_id)
);

CREATE TABLE IF NOT EXISTS service_lifecycle_argument (
    circuit_id                TEXT NOT NULL,
    service_id                TEXT NOT NULL,
    key                       TEXT NOT NULL,
    value                     TEXT NOT NULL,
    position                  INTEGER NOT NULL,
    PRIMARY KEY (circuit_id, service_id, key)
);
