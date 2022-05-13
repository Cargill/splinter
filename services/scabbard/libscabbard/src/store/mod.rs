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

//! Stores required for a scabbard services operation.

#[cfg(feature = "scabbardv3")]
mod command;
mod commit_hash;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub(crate) mod pool;
#[cfg(feature = "scabbardv3-store")]
mod scabbard_store;

#[cfg(feature = "scabbardv3")]
pub use command::{
    ScabbardFinalizeServiceCommand, ScabbardPrepareServiceCommand, ScabbardPurgeServiceCommand,
    ScabbardRetireServiceCommand,
};

#[cfg(feature = "diesel")]
pub use commit_hash::diesel;
pub use commit_hash::transact;
pub use commit_hash::{CommitHashStore, CommitHashStoreError};

#[cfg(all(feature = "scabbardv3-store", feature = "diesel"))]
pub use scabbard_store::DieselScabbardStore;
#[cfg(feature = "scabbardv3-store")]
pub use scabbard_store::PooledScabbardStoreFactory;
#[cfg(feature = "scabbardv3-store")]
pub use scabbard_store::{
    Action, AlarmType, CommitEntry, CommitEntryBuilder, ConsensusAction, ConsensusContext,
    ConsensusDecision, ConsensusEvent, ConsensusType, Context, ContextBuilder, Event, Identified,
    Message, Notification, Participant, ScabbardService, ScabbardServiceBuilder, ScabbardStore,
    ScabbardStoreFactory, ServiceStatus, State,
};
#[cfg(all(feature = "scabbardv3-store", feature = "postgres"))]
pub use scabbard_store::{PgScabbardStoreFactory, PooledPgScabbardStoreFactory};
#[cfg(all(feature = "scabbardv3-store", feature = "sqlite"))]
pub use scabbard_store::{PooledSqliteScabbardStoreFactory, SqliteScabbardStoreFactory};
