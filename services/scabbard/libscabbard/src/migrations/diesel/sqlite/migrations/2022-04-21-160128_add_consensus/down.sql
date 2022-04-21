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

-- Rename old table
ALTER TABLE scabbard_service RENAME TO _scabbard_service_old;

-- create new table without consensus
CREATE TABLE IF NOT EXISTS scabbard_service (
    service_id       TEXT PRIMARY KEY NOT NULL,
    status           TEXT NOT NULL
    CHECK ( status IN ('PREPARED', 'FINALIZED', 'RETIRED') )
);

-- move data from old table
INSERT INTO scabbard_service (service_id, status)
  SELECT service_id, status
  FROM _scabbard_service_old;

-- drop old table
DROP TABLE _scabbard_service_old;
