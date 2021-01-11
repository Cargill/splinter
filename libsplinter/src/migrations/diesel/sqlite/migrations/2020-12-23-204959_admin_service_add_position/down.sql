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

ALTER TABLE proposed_node DROP COLUMN position;

ALTER TABLE proposed_node_endpoint DROP COLUMN position;

ALTER TABLE proposed_service DROP COLUMN position;

ALTER TABLE proposed_service_argument DROP COLUMN position;

ALTER TABLE vote_record DROP COLUMN position;

ALTER TABLE service DROP COLUMN position;

ALTER TABLE service_argument DROP COLUMN position;

ALTER TABLE circuit_member DROP COLUMN position;
