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

ALTER TABLE scabbard_alarm DROP CONSTRAINT scabbard_alarm_service_id_fkey;
ALTER TABLE scabbard_peer DROP CONSTRAINT scabbard_peer_service_id_fkey;
ALTER TABLE scabbard_peer DROP CONSTRAINT scabbard_peer_circuit_id_service_id_fkey;
ALTER TABLE scabbard_v3_commit_history DROP CONSTRAINT scabbard_v3_commit_history_circuit_id_service_id_fkey;
ALTER TABLE consensus_2pc_context DROP CONSTRAINT consensus_2pc_context_circuit_id_service_id_fkey;
ALTER TABLE consensus_2pc_context_participant DROP CONSTRAINT consensus_2pc_context_participant_circuit_id_service_id_fkey;
ALTER TABLE consensus_2pc_action DROP CONSTRAINT consensus_2pc_action_circuit_id_service_id_fkey;
ALTER TABLE consensus_2pc_event DROP CONSTRAINT consensus_2pc_event_circuit_id_service_id_fkey;
ALTER TABLE scabbard_alarm DROP CONSTRAINT scabbard_alarm_circuit_id_service_id_fkey;


-- Recreate the foreign key constraints with on delete cascade
ALTER TABLE scabbard_alarm ADD CONSTRAINT scabbard_alarm_service_id_fkey
  FOREIGN KEY (circuit_id, service_id) REFERENCES scabbard_service(circuit_id, service_id) ON DELETE CASCADE;
ALTER TABLE scabbard_peer ADD CONSTRAINT scabbard_peer_service_id_fkey
  FOREIGN KEY (circuit_id, service_id) REFERENCES scabbard_service(circuit_id, service_id) ON DELETE CASCADE;
ALTER TABLE scabbard_peer
  ADD CONSTRAINT scabbard_peer_circuit_id_service_id_fkey
  FOREIGN KEY(circuit_id, service_id) REFERENCES scabbard_service(circuit_id, service_id) ON DELETE CASCADE;
ALTER TABLE scabbard_v3_commit_history
  ADD CONSTRAINT scabbard_v3_commit_history_circuit_id_service_id_fkey
  FOREIGN KEY(circuit_id, service_id) REFERENCES scabbard_service(circuit_id, service_id) ON DELETE CASCADE;
ALTER TABLE consensus_2pc_context
  ADD CONSTRAINT consensus_2pc_context_circuit_id_service_id_fkey
  FOREIGN KEY(circuit_id, service_id) REFERENCES scabbard_service(circuit_id, service_id) ON DELETE CASCADE;
ALTER TABLE consensus_2pc_context_participant
  ADD CONSTRAINT consensus_2pc_context_participant_circuit_id_service_id_fkey
  FOREIGN KEY(circuit_id, service_id) REFERENCES scabbard_service(circuit_id, service_id) ON DELETE CASCADE;
ALTER TABLE consensus_2pc_action
  ADD CONSTRAINT consensus_2pc_action_circuit_id_service_id_fkey
  FOREIGN KEY(circuit_id, service_id) REFERENCES scabbard_service(circuit_id, service_id) ON DELETE CASCADE;
ALTER TABLE consensus_2pc_event
  ADD CONSTRAINT consensus_2pc_event_circuit_id_service_id_fkey
  FOREIGN KEY(circuit_id, service_id) REFERENCES scabbard_service(circuit_id, service_id) ON DELETE CASCADE;
ALTER TABLE scabbard_alarm
  ADD CONSTRAINT scabbard_alarm_circuit_id_service_id_fkey
  FOREIGN KEY(circuit_id, service_id) REFERENCES scabbard_service(circuit_id, service_id) ON DELETE CASCADE;
