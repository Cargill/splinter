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

-- DROP the executed_epoch column
ALTER TABLE consensus_2pc_event DROP COLUMN executed_epoch;

-- Revert the executed_at column to be BIGINT of unix epoch milliseconds
ALTER TABLE consensus_2pc_event ADD COLUMN temp_executed_at BIGINT NULL;

UPDATE consensus_2pc_event
   SET temp_executed_at = (extract(epoch from executed_at) * 1000)::BIGINT
   WHERE executed_at IS NOT NULL;

ALTER TABLE consensus_2pc_event
  ALTER COLUMN executed_at TYPE BIGINT USING temp_executed_at;

ALTER TABLE consensus_2pc_event DROP COLUMN temp_executed_at;
