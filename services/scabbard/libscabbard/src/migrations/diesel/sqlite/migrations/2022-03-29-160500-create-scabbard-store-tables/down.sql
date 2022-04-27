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

DROP TABLE IF EXISTS consensus_2pc_consensus_coordinator_context;
DROP TABLE IF EXISTS consensus_2pc_consensus_coordinator_context_participant;
DROP TABLE IF EXISTS consensus_2pc_action;
DROP TABLE IF EXISTS consensus_2pc_update_coordinator_context_action;
DROP TABLE IF EXISTS consensus_2pc_coordinator_send_message_action;
DROP TABLE IF EXISTS consensus_2pc_coordinator_notification_action;
DROP TABLE IF EXISTS consensus_2pc_update_coordinator_context_action_participant;
DROP TABLE IF EXISTS consensus_2pc_participant_context;
DROP TABLE IF EXISTS consensus_2pc_participant_context_participant;
DROP TABLE IF EXISTS consensus_2pc_update_participant_context_action;
DROP TABLE IF EXISTS consensus_2pc_update_participant_context_action_participant;
DROP TABLE IF EXISTS consensus_2pc_participant_send_message_action;
DROP TABLE IF EXISTS consensus_2pc_participant_notification_action;
