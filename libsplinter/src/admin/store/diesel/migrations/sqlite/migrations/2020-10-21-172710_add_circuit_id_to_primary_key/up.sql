-- Your SQL goes here---- Copyright 2018-2020 Cargill Incorporated
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

-- Add circuit id to the primary key, node_id and endpoint is not unique in
-- proposed nodes.
CREATE TABLE IF NOT EXISTS proposed_node_endpoint_copy (
    node_id                TEXT NOT NULL,
    endpoint               TEXT NOT NULL,
    circuit_id             TEXT NOT NULL,
    PRIMARY KEY (circuit_id, node_id, endpoint),
    FOREIGN KEY (circuit_id) REFERENCES proposed_circuit(circuit_id) ON DELETE CASCADE
);

INSERT INTO proposed_node_endpoint_copy(node_id, endpoint, circuit_id)
   SELECT node_id, endpoint, circuit_id FROM proposed_node_endpoint;
DROP TABLE proposed_node_endpoint;
ALTER TABLE proposed_node_endpoint_copy RENAME TO proposed_node_endpoint;
