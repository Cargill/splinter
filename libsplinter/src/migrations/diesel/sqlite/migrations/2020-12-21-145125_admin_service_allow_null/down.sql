---- Copyright 2018-2020 Cargill Incorporated
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

---- Copyright 2018-2020 Cargill Incorporated
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

-- Allow update
CREATE TABLE IF NOT EXISTS proposed_circuit_copy (
    circuit_id                TEXT NOT NULL,
    authorization_type        TEXT NOT NULL,
    persistence               TEXT NOT NULL,
    durability                TEXT NOT NULL,
    routes                    TEXT NOT NULL,
    circuit_management_type   TEXT NOT NULL,
    application_metadata      BINARY NOT NULL DEFAULT "",
    comments                  TEXT NOT NULL DEFAULT "",
    display_name              TEXT,
    PRIMARY KEY (circuit_id),
    FOREIGN KEY (circuit_id) REFERENCES circuit_proposal(circuit_id) ON DELETE CASCADE
);

INSERT INTO proposed_circuit_copy(circuit_id, authorization_type, persistence,
    durability, routes, circuit_management_type, comments, display_name)
   SELECT circuit_id, authorization_type, persistence, durability, routes,
   circuit_management_type, comments, display_name FROM proposed_circuit;
DROP TABLE proposed_circuit;
ALTER TABLE proposed_circuit_copy RENAME TO proposed_circuit;
