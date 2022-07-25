-- Copyright 2018-2022 Cargill Incorporated
--
-- Licensed under the Apache License, Version 2.0 (the "License");
-- you may not use this file except in compliance with the Licens
-- You may obtain a copy of the License at
--
--     http://www.apachorg/licenses/LICENSE-2.0
--
-- Unless required by applicable law or agreed to in writing, software
-- distributed under the License is distributed on an "AS IS" BASIS,
-- WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
-- See the License for the specific language governing permissions and
-- limitations under the Licens
-- -----------------------------------------------------------------------------

PRAGMA foreign_keys=off;

-- add a new column to consensus_2pc_event
ALTER TABLE consensus_2pc_event ADD COLUMN update_context_action_id INTEGER;

-- set the consensus_2pc_event column for each event based on 
UPDATE consensus_2pc_event SET update_context_action_id = 
CASE consensus_2pc_event.executed_at
    WHEN NULL THEN NULL
    ELSE (
        SELECT a.id
        FROM consensus_2pc_action a, consensus_2pc_update_context_action uc
        WHERE a.id = uc.action_id
            AND consensus_2pc_event.circuit_id == a.circuit_id
            AND consensus_2pc_event.service_id == a.service_id
            AND a.executed_at < consensus_2pc_event.executed_at
        ORDER BY a.executed_at DESC
        LIMIT 1
    )
    END;
