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

ALTER TABLE admin_event_proposed_node
ADD COLUMN position INTEGER NOT NULL DEFAULT 0;

ALTER TABLE admin_event_proposed_node_endpoint
ADD COLUMN position INTEGER NOT NULL DEFAULT 0;

ALTER TABLE admin_event_proposed_service
ADD COLUMN position INTEGER NOT NULL DEFAULT 0;

ALTER TABLE admin_event_proposed_service_argument
ADD COLUMN position INTEGER NOT NULL DEFAULT 0;

ALTER TABLE admin_event_vote_record
ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
