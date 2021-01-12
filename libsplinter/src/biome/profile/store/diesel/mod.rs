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

//! Database-backed implementation of the [UserProfileStore], powered by [diesel].

pub(in crate::biome) mod models;
mod operations;
pub(in crate::biome) mod schema;

use diesel::r2d2::{ConnectionManager, Pool};

use super::{Profile, UserProfileStore, UserProfileStoreError};

use models::ProfileModel;

use operations::{
    add_profile::UserProfileStoreAddProfile as _, get_profile::UserProfileStoreGetProfile as _,
    list_profiles::UserProfileStorelistProfiles as _,
    remove_profile::UserProfileStoreRemoveProfile as _,
    update_profile::UserProfileStoreUpdateProfile as _, UserProfileStoreOperations,
};

/// Manages creating, updating, and fetching profiles from the database
pub struct DieselUserProfileStore<C: diesel::Connection + 'static> {
    connection_pool: Pool<ConnectionManager<C>>,
}

impl<C: diesel::Connection> DieselUserProfileStore<C> {
    /// Creates a new DieselUserProfileStore
    ///
    /// # Arguments
    ///
    ///  * `connection_pool`: connection pool to the database
    pub fn new(connection_pool: Pool<ConnectionManager<C>>) -> Self {
        DieselUserProfileStore { connection_pool }
    }
}

#[cfg(feature = "postgres")]
impl UserProfileStore for DieselUserProfileStore<diesel::pg::PgConnection> {
    fn add_profile(&self, profile: Profile) -> Result<(), UserProfileStoreError> {
        let connection = self.connection_pool.get()?;
        UserProfileStoreOperations::new(&*connection).add_profile(profile)
    }

    fn update_profile(&self, profile: Profile) -> Result<(), UserProfileStoreError> {
        let connection = self.connection_pool.get()?;
        UserProfileStoreOperations::new(&*connection).update_profile(profile)
    }

    fn remove_profile(&self, user_id: &str) -> Result<(), UserProfileStoreError> {
        let connection = self.connection_pool.get()?;
        UserProfileStoreOperations::new(&*connection).remove_profile(user_id)
    }

    fn get_profile(&self, user_id: &str) -> Result<Profile, UserProfileStoreError> {
        let connection = self.connection_pool.get()?;
        UserProfileStoreOperations::new(&*connection).get_profile(user_id)
    }

    fn list_profiles(&self) -> Result<Option<Vec<Profile>>, UserProfileStoreError> {
        let connection = self.connection_pool.get()?;
        UserProfileStoreOperations::new(&*connection).list_profiles()
    }

    fn clone_box(&self) -> Box<dyn UserProfileStore> {
        Box::new(Self {
            connection_pool: self.connection_pool.clone(),
        })
    }
}

#[cfg(feature = "sqlite")]
impl UserProfileStore for DieselUserProfileStore<diesel::sqlite::SqliteConnection> {
    fn add_profile(&self, profile: Profile) -> Result<(), UserProfileStoreError> {
        let connection = self.connection_pool.get()?;
        UserProfileStoreOperations::new(&*connection).add_profile(profile)
    }

    fn update_profile(&self, profile: Profile) -> Result<(), UserProfileStoreError> {
        let connection = self.connection_pool.get()?;
        UserProfileStoreOperations::new(&*connection).update_profile(profile)
    }

    fn remove_profile(&self, user_id: &str) -> Result<(), UserProfileStoreError> {
        let connection = self.connection_pool.get()?;
        UserProfileStoreOperations::new(&*connection).remove_profile(user_id)
    }

    fn get_profile(&self, user_id: &str) -> Result<Profile, UserProfileStoreError> {
        let connection = self.connection_pool.get()?;
        UserProfileStoreOperations::new(&*connection).get_profile(user_id)
    }

    fn list_profiles(&self) -> Result<Option<Vec<Profile>>, UserProfileStoreError> {
        let connection = self.connection_pool.get()?;
        UserProfileStoreOperations::new(&*connection).list_profiles()
    }

    fn clone_box(&self) -> Box<dyn UserProfileStore> {
        Box::new(Self {
            connection_pool: self.connection_pool.clone(),
        })
    }
}

impl From<ProfileModel> for Profile {
    fn from(user_profile: ProfileModel) -> Self {
        Self {
            user_id: user_profile.user_id,
            name: user_profile.name,
            given_name: user_profile.given_name,
            family_name: user_profile.family_name,
            email: user_profile.email,
            picture: user_profile.picture,
        }
    }
}

#[cfg(all(test, feature = "sqlite"))]
pub mod tests {
    use super::*;

    use crate::biome::profile::store::ProfileBuilder;
    use crate::migrations::run_sqlite_migrations;

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };

    /// Verify that a SQLite-backed `DieselUserProfileStore` correctly supports adding and getting profiles.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create the `DieselUserProfileStore`.
    /// 3. Add a profile.
    /// 4. Verify that the `get_profile` method returns correct values for all profile fields.
    /// 5. Verify that the `get_profile` method returns an error when given an nonexistent user_id.
    #[test]
    fn sqlite_add_and_get_profile() {
        let pool = create_connection_pool_and_migrate();

        let user_profile_store = DieselUserProfileStore::new(pool);

        let user_id = "user_id".to_string();
        let name = Some("name".to_string());

        let profile = ProfileBuilder::new()
            .with_user_id(user_id.clone())
            .with_name(name)
            .with_given_name(None)
            .with_family_name(None)
            .with_email(None)
            .with_picture(None)
            .build()
            .expect("Unable to build profile");
        user_profile_store
            .add_profile(profile)
            .expect("Unable to add profile");

        let profile = user_profile_store
            .get_profile(&user_id.clone())
            .expect("Unable to get profile");

        assert_eq!(profile.user_id(), &user_id);
        assert_eq!(profile.name(), Some("name"));
        assert_eq!(profile.given_name(), None);
        assert_eq!(profile.family_name(), None);
        assert_eq!(profile.email(), None);
        assert_eq!(profile.picture(), None);

        assert!(user_profile_store.get_profile("InvalidID").is_err());
    }

    /// Verify that a SQLite-backed `DieselUserProfileStore` correctly supports listing profiles.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create the `DieselUserProfileStore`.
    /// 3. Add a profile.
    /// 4. Verify that the `list_profiles` method returns a vector containing the added profile.
    /// 5. Verify that all values of the returned profile are correct.
    #[test]
    fn sqlite_list_profiles() {
        let pool = create_connection_pool_and_migrate();

        let user_profile_store = DieselUserProfileStore::new(pool);

        let user_id = "user_id".to_string();
        let name = Some("name".to_string());

        let profile = ProfileBuilder::new()
            .with_user_id(user_id.clone())
            .with_name(name)
            .with_given_name(None)
            .with_family_name(None)
            .with_email(None)
            .with_picture(None)
            .build()
            .expect("Unable to build profile");
        user_profile_store
            .add_profile(profile)
            .expect("Unable to add profile");

        let profiles = user_profile_store
            .list_profiles()
            .expect("Unable to get profiles");
        let profile = &profiles.unwrap()[0];

        assert_eq!(profile.user_id(), "user_id");
        assert_eq!(profile.name(), Some("name"));
        assert!(profile.given_name().is_none());
        assert!(profile.family_name().is_none());
        assert!(profile.email().is_none());
        assert!(profile.picture().is_none());
    }

    /// Verify that a SQLite-backed `DieselUserProfileStore` correctly supports updating profiles.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create the `DieselUserProfileStore`.
    /// 3. Add a profile.
    /// 4. Create an updated profile with the same `user_id` as the first.
    /// 5. Call `update_profile` on the store with the updated profile as the argument.
    /// 6. Verify that the `get_profile` method returns the profile with the updated fields.
    /// 7. Create a profile with a different `user_id`.
    /// 8. Call `update_profile` on the store with the new profile.
    /// 9. Verify an error is returned because there is no profile in the store with the given `user_id`.
    #[test]
    fn sqlite_update_profile() {
        let pool = create_connection_pool_and_migrate();

        let user_profile_store = DieselUserProfileStore::new(pool);

        let user_id = "user_id".to_string();
        let name = Some("name".to_string());

        let profile = ProfileBuilder::new()
            .with_user_id(user_id.clone())
            .with_name(name)
            .with_given_name(None)
            .with_family_name(None)
            .with_email(None)
            .with_picture(None)
            .build()
            .expect("Unable to build profile");
        user_profile_store
            .add_profile(profile)
            .expect("Unable to add profile");

        let updated_profile = ProfileBuilder::new()
            .with_user_id(user_id.clone())
            .with_name(Some("New Name".to_string()))
            .with_given_name(Some("New".to_string()))
            .with_family_name(Some("Name".to_string()))
            .with_email(None)
            .with_picture(None)
            .build()
            .expect("Unable to build updated profile");

        user_profile_store
            .update_profile(updated_profile)
            .expect("Unable to update profile");

        let updated_profile = user_profile_store
            .get_profile(&user_id.clone())
            .expect("Unable to get updated profile");

        assert_eq!(updated_profile.user_id(), "user_id");
        assert_eq!(updated_profile.name(), Some("New Name"));
        assert_eq!(updated_profile.given_name(), Some("New"));
        assert_eq!(updated_profile.family_name(), Some("Name"));
        assert!(updated_profile.email().is_none());
        assert!(updated_profile.picture().is_none());

        let bad_profile = ProfileBuilder::new()
            .with_user_id("bad_id".to_string())
            .with_name(None)
            .with_given_name(None)
            .with_family_name(None)
            .with_email(None)
            .with_picture(None)
            .build()
            .expect("Unable to build profile");

        assert!(user_profile_store.update_profile(bad_profile).is_err());
    }

    /// Verify that a SQLite-backed `DieselUserProfileStore` correctly supports removing profiles.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create the `DieselUserProfileStore`.
    /// 3. Add a profile.
    /// 4. Call `remove_profile` on the store.
    /// 5. Verify that calling `get_profile` on the store with the `user_id` of the previously
    ///    added profile returns an error.
    #[test]
    fn sqlite_remove_profile() {
        let pool = create_connection_pool_and_migrate();

        let user_profile_store = DieselUserProfileStore::new(pool);

        let user_id = "user_id".to_string();
        let name = Some("name".to_string());

        let profile = ProfileBuilder::new()
            .with_user_id(user_id.clone())
            .with_name(name)
            .with_given_name(None)
            .with_family_name(None)
            .with_email(None)
            .with_picture(None)
            .build()
            .expect("Unable to build profile");
        user_profile_store
            .add_profile(profile)
            .expect("Unable to add profile");

        user_profile_store
            .remove_profile(&user_id.clone())
            .expect("Unable to remove profile");

        assert!(user_profile_store.get_profile("user_id").is_err());
    }

    /// Creates a connection pool for an in-memory SQLite database with only a single connection
    /// available. Each connection is backed by a different in-memory SQLite database, so limiting
    /// the pool to a single connection insures that the same DB is used for all operations.
    fn create_connection_pool_and_migrate() -> Pool<ConnectionManager<SqliteConnection>> {
        let connection_manager = ConnectionManager::<SqliteConnection>::new(":memory:");
        let pool = Pool::builder()
            .max_size(1)
            .build(connection_manager)
            .expect("Failed to build connection pool");

        run_sqlite_migrations(&*pool.get().expect("Failed to get connection for migrations"))
            .expect("Failed to run migrations");

        pool
    }
}
