// Copyright 2018-2021 Cargill Incorporated
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

use std::error::Error;

use transact::database::error::DatabaseError;
use transact::execution::adapter::ExecutionAdapterError;
use transact::execution::executor::ExecutorError;
use transact::protocol::batch::BatchBuildError;
use transact::scheduler::SchedulerError;
use transact::state::error::StateWriteError;
use transact::state::merkle::StateDatabaseError;

#[derive(Debug)]
pub struct ScabbardStateError(pub String);

impl Error for ScabbardStateError {}

impl std::fmt::Display for ScabbardStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "scabbard state error: {}", self.0)
    }
}

impl From<BatchBuildError> for ScabbardStateError {
    fn from(err: BatchBuildError) -> Self {
        ScabbardStateError(err.to_string())
    }
}

impl From<DatabaseError> for ScabbardStateError {
    fn from(err: DatabaseError) -> Self {
        ScabbardStateError(err.to_string())
    }
}

impl From<ExecutionAdapterError> for ScabbardStateError {
    fn from(err: ExecutionAdapterError) -> Self {
        ScabbardStateError(err.to_string())
    }
}

impl From<ExecutorError> for ScabbardStateError {
    fn from(err: ExecutorError) -> Self {
        ScabbardStateError(err.to_string())
    }
}

impl From<SchedulerError> for ScabbardStateError {
    fn from(err: SchedulerError) -> Self {
        ScabbardStateError(err.to_string())
    }
}

impl From<StateDatabaseError> for ScabbardStateError {
    fn from(err: StateDatabaseError) -> Self {
        ScabbardStateError(err.to_string())
    }
}

impl From<StateWriteError> for ScabbardStateError {
    fn from(err: StateWriteError) -> Self {
        ScabbardStateError(err.to_string())
    }
}

#[derive(Debug)]
pub enum StateSubscriberError {
    UnableToHandleEvent(String),
    Unsubscribe,
}

impl Error for StateSubscriberError {}

impl std::fmt::Display for StateSubscriberError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            StateSubscriberError::UnableToHandleEvent(msg) => {
                write!(f, "unable to handle event: {}", msg)
            }
            StateSubscriberError::Unsubscribe => f.write_str("unsubscribe"),
        }
    }
}
