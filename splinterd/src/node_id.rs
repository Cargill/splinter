// Copyright 2018-2022 Cargill Incorporated
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use rand::Rng;
use splinter::error::InternalError;
use splinter::node_id::store::NodeIdStore;

use crate::error::UserError;

pub fn get_node_id(
    passed_in_node_id: Option<String>,
    node_id_store: Box<dyn NodeIdStore>,
) -> Result<String, UserError> {
    get_from_store(passed_in_node_id, node_id_store)
}

fn get_random_node_id() -> String {
    format!("n{}", rand::thread_rng().gen::<u16>())
}

fn get_from_store(
    config_node_id: Option<String>,
    node_id_store: Box<dyn NodeIdStore>,
) -> Result<String, UserError> {
    let save_new_node_id = |node_id| -> Result<(), UserError> {
        node_id_store
            .set_node_id(node_id)
            .map_err(|err| UserError::from(InternalError::from_source(Box::new(err))))
    };

    match (node_id_store.get_node_id(), config_node_id) {
        (Ok(Some(db_node_id)), Some(config_node_id)) => {
            if db_node_id == config_node_id {
                Ok(db_node_id)
            } else {
                Err(UserError::InvalidArgument(format!(
                    "node_id from database {} does not match node_id from config {}",
                    db_node_id, config_node_id
                )))
            }
        }
        (Ok(Some(db_node_id)), None) => Ok(db_node_id),
        (Ok(None), Some(config_node_id)) => {
            save_new_node_id(config_node_id.clone())?;
            Ok(config_node_id)
        }
        (Ok(None), None) => {
            let node_id = get_random_node_id();
            save_new_node_id(node_id.clone())?;
            Ok(node_id)
        }
        (Err(err), _) => Err(UserError::from(InternalError::from_source(Box::new(err)))),
    }
}
