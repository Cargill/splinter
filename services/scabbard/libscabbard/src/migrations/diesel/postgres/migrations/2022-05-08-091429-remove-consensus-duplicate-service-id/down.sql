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

ALTER TABLE consensus_2pc_update_context_action ADD COLUMN service_id TEXT NOT NULL;

ALTER TABLE consensus_2pc_send_message_action ADD COLUMN service_id TEXT NOT NULL;

ALTER TABLE consensus_2pc_notification_action ADD COLUMN service_id TEXT NOT NULL;

ALTER TABLE consensus_2pc_update_context_action_participant ADD COLUMN service_id TEXT NOT NULL;

ALTER TABLE consensus_2pc_deliver_event ADD COLUMN service_id TEXT NOT NULL;

ALTER TABLE consensus_2pc_start_event ADD COLUMN service_id TEXT NOT NULL;

ALTER TABLE consensus_2pc_vote_event ADD COLUMN service_id TEXT NOT NULL;

ALTER TABLE consensus_2pc_update_context_action
 ADD CONSTRAINT service_id FOREIGN KEY (service_id) REFERENCES consensus_2pc_context(service_id);
ALTER TABLE consensus_2pc_send_message_action 
 ADD CONSTRAINT service_id FOREIGN KEY (service_id) REFERENCES consensus_2pc_context(service_id);
ALTER TABLE consensus_2pc_notification_action 
 ADD CONSTRAINT service_id FOREIGN KEY (service_id) REFERENCES consensus_2pc_context(service_id);
ALTER TABLE consensus_2pc_deliver_event
 ADD CONSTRAINT service_id FOREIGN KEY (service_id) REFERENCES consensus_2pc_context(service_id);
ALTER TABLE consensus_2pc_start_event
 ADD CONSTRAINT service_id FOREIGN KEY (service_id) REFERENCES consensus_2pc_context(service_id);
ALTER TABLE consensus_2pc_vote_event
 ADD CONSTRAINT service_id FOREIGN KEY (service_id) REFERENCES consensus_2pc_context(service_id);
