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

ALTER TABLE consensus_2pc_update_context_action_participant
DROP CONSTRAINT consensus_2pc_update_context_action_participant_pkey;

ALTER TABLE consensus_2pc_update_context_action_participant
ADD CONSTRAINT consensus_2pc_update_context_action_participant_pkey
PRIMARY KEY (action_id, process);
