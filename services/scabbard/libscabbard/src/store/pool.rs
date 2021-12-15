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

use std::sync::{Arc, RwLock};

use diesel::r2d2::{ConnectionManager, Pool};
use splinter::error::InternalError;

pub enum ConnectionPool<C: diesel::Connection + 'static> {
    Normal(Pool<ConnectionManager<C>>),
    WriteExclusive(Arc<RwLock<Pool<ConnectionManager<C>>>>),
}

macro_rules! conn {
    ($pool:ident) => {
        $pool
            .get()
            .map_err(|e| InternalError::from_source(Box::new(e)))
    };
}

impl<C: diesel::Connection> ConnectionPool<C> {
    pub fn execute_write<F, T, E>(&self, f: F) -> Result<T, E>
    where
        F: FnOnce(&C) -> Result<T, E>,
        E: From<InternalError>,
    {
        match self {
            Self::Normal(pool) => f(&*conn!(pool)?),
            Self::WriteExclusive(locked_pool) => locked_pool
                .write()
                .map_err(|_| {
                    InternalError::with_message("Connection pool rwlock is poisoned".into()).into()
                })
                .and_then(|pool| f(&*conn!(pool)?)),
        }
    }

    pub fn execute_read<F, T, E>(&self, f: F) -> Result<T, E>
    where
        F: FnOnce(&C) -> Result<T, E>,
        E: From<InternalError>,
    {
        match self {
            Self::Normal(pool) => f(&*conn!(pool)?),
            Self::WriteExclusive(locked_pool) => locked_pool
                .read()
                .map_err(|_| {
                    InternalError::with_message("Connection pool rwlock is poisoned".into()).into()
                })
                .and_then(|pool| f(&*conn!(pool)?)),
        }
    }
}

impl<C: diesel::Connection> Clone for ConnectionPool<C> {
    fn clone(&self) -> Self {
        match self {
            Self::Normal(pool) => Self::Normal(pool.clone()),
            Self::WriteExclusive(locked_pool) => Self::WriteExclusive(locked_pool.clone()),
        }
    }
}

impl<C: diesel::Connection> From<Pool<ConnectionManager<C>>> for ConnectionPool<C> {
    fn from(pool: Pool<ConnectionManager<C>>) -> Self {
        Self::Normal(pool)
    }
}

impl<C: diesel::Connection> From<Arc<RwLock<Pool<ConnectionManager<C>>>>> for ConnectionPool<C> {
    fn from(pool: Arc<RwLock<Pool<ConnectionManager<C>>>>) -> Self {
        Self::WriteExclusive(pool)
    }
}
