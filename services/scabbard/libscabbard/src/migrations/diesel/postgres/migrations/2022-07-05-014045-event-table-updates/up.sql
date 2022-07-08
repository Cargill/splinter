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

-- Add the executed_epoch column
ALTER TABLE consensus_2pc_event
    ADD COLUMN executed_epoch BIGINT;

-- Default all executed columns to the most recent epoch
UPDATE consensus_2pc_event
SET executed_epoch = ctx.epoch
FROM consensus_2pc_context ctx
WHERE ctx.circuit_id = consensus_2pc_event.circuit_id
  AND ctx.service_id = consensus_2pc_event.service_id
  AND consensus_2pc_event.executed_at IS NOT NULL;

-- Change the executed_at column to be a timestamp data type.
ALTER TABLE consensus_2pc_event ADD COLUMN temp_executed_at TIMESTAMP without time zone NULL;

UPDATE consensus_2pc_event
   SET temp_executed_at = TO_TIMESTAMP(executed_at::double precision / 1000)
   WHERE executed_at IS NOT NULL;

ALTER TABLE consensus_2pc_event
  ALTER COLUMN executed_at TYPE TIMESTAMP without time zone USING temp_executed_at;

ALTER TABLE consensus_2pc_event DROP COLUMN temp_executed_at;
