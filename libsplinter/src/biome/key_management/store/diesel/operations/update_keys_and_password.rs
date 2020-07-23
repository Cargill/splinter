// Copyright 2018-2020 Cargill Incorporated
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

use super::KeyStoreOperations;
use crate::biome::credentials::store::diesel::schema::user_credentials;
use crate::biome::key_management::store::diesel::models::KeyModel;
use crate::biome::key_management::store::diesel::schema::keys;
use crate::biome::key_management::{store::KeyStoreError, Key};

use diesel::{
    dsl::{delete, insert_into},
    prelude::*,
    result::{DatabaseErrorKind, Error as QueryError},
};

pub(in crate::biome::key_management) trait KeyStoreUpdateKeysAndPasswordOperation {
    fn update_keys_and_password(
        &self,
        user_id: &str,
        updated_password: &str,
        keys: &[Key],
    ) -> Result<(), KeyStoreError>;
}

#[cfg(feature = "postgres")]
impl<'a> KeyStoreUpdateKeysAndPasswordOperation
    for KeyStoreOperations<'a, diesel::pg::PgConnection>
{
    fn update_keys_and_password(
        &self,
        user_id: &str,
        updated_password: &str,
        keys: &[Key],
    ) -> Result<(), KeyStoreError> {
        let replacement_keys = keys
            .iter()
            .map(|key| key.clone().into())
            .collect::<Vec<KeyModel>>();

        self.conn
            .transaction::<(), _, _>(|| {
                if let Err(err) =
                    delete(keys::table.filter(keys::user_id.eq(user_id))).execute(self.conn)
                {
                    return Err(err);
                }
                if let Err(err) = insert_into(keys::table)
                    .values(replacement_keys)
                    .execute(self.conn)
                {
                    return Err(err);
                }
                if let Err(err) = diesel::update(
                    user_credentials::table.filter(user_credentials::user_id.eq(&user_id)),
                )
                .set(user_credentials::password.eq(&updated_password))
                .execute(self.conn)
                {
                    return Err(err);
                }

                Ok(())
            })
            .map_err(|err| {
                if let QueryError::DatabaseError(db_err, _) = err {
                    match db_err {
                        DatabaseErrorKind::UniqueViolation => {
                            return KeyStoreError::DuplicateKeyError(format!(
                                "Public key for user {} is already in database",
                                user_id
                            ));
                        }
                        DatabaseErrorKind::ForeignKeyViolation => {
                            return KeyStoreError::UserDoesNotExistError(format!(
                                "User with ID {} does not exist in database",
                                user_id
                            ));
                        }
                        _ => {
                            return KeyStoreError::OperationError {
                                context: "Failed to add key".to_string(),
                                source: Box::new(err),
                            }
                        }
                    }
                }
                KeyStoreError::OperationError {
                    context: "Failed to add key".to_string(),
                    source: Box::new(err),
                }
            })?;

        Ok(())
    }
}

#[cfg(feature = "sqlite")]
impl<'a> KeyStoreUpdateKeysAndPasswordOperation
    for KeyStoreOperations<'a, diesel::sqlite::SqliteConnection>
{
    fn update_keys_and_password(
        &self,
        user_id: &str,
        updated_password: &str,
        keys: &[Key],
    ) -> Result<(), KeyStoreError> {
        let replacement_keys = keys
            .iter()
            .map(|key| key.clone().into())
            .collect::<Vec<KeyModel>>();

        self.conn
            .transaction::<(), _, _>(|| {
                if let Err(err) =
                    delete(keys::table.filter(keys::user_id.eq(user_id))).execute(self.conn)
                {
                    return Err(err);
                }
                if let Err(err) = insert_into(keys::table)
                    .values(replacement_keys)
                    .execute(self.conn)
                {
                    return Err(err);
                }
                if let Err(err) = diesel::update(
                    user_credentials::table.filter(user_credentials::user_id.eq(&user_id)),
                )
                .set(user_credentials::password.eq(&updated_password))
                .execute(self.conn)
                {
                    return Err(err);
                }

                Ok(())
            })
            .map_err(|err| {
                if let QueryError::DatabaseError(db_err, _) = err {
                    match db_err {
                        DatabaseErrorKind::UniqueViolation => {
                            return KeyStoreError::DuplicateKeyError(format!(
                                "Public key for user {} is already in database",
                                user_id
                            ));
                        }
                        DatabaseErrorKind::ForeignKeyViolation => {
                            return KeyStoreError::UserDoesNotExistError(format!(
                                "User with ID {} does not exist in database",
                                user_id
                            ));
                        }
                        _ => {
                            return KeyStoreError::OperationError {
                                context: "Failed to add key".to_string(),
                                source: Box::new(err),
                            }
                        }
                    }
                }
                KeyStoreError::OperationError {
                    context: "Failed to add key".to_string(),
                    source: Box::new(err),
                }
            })?;

        Ok(())
    }
}
