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

use std::env;
use std::error::Error;
use std::panic;
use std::sync::atomic::{AtomicUsize, Ordering};

use diesel::connection::SimpleConnection;
use diesel::prelude::*;

use crate::migrations::run_postgres_migrations;

/// Execute a test against a postgres database.
///
/// This function will create a database, based on the name of the current test being run, if
/// known. It will then run the migrations against the new database.  After the test completes, it
/// will drop the test datbase previously created, regardless of success or failure of the test.
///
/// The base url for a postgres server is specified by the environment variable
/// `DIESEL_POSTGRES_TEST_URL` or defaults to `"postgres://postgres:test@localhost:5432"`.
pub fn run_postgres_test<T>(test: T) -> Result<(), Box<dyn Error>>
where
    T: FnOnce(&str) -> Result<(), Box<dyn Error>> + panic::UnwindSafe,
{
    let (drop_tables_res, test_result) = {
        let base_url = match env::var("DIESEL_POSTGRES_TEST_URL").ok() {
            Some(url) => url,
            None => {
                println!(
                    "Ignoring {}",
                    std::thread::current().name().unwrap_or("<unknown test>")
                );
                return Ok(());
            }
        };

        let db_name = db_name();
        {
            let conn = PgConnection::establish(&base_url)?;
            conn.batch_execute(&format!("create database {};", db_name))?;
        }

        let url = format!("{}/{}", base_url, db_name);
        {
            let conn = PgConnection::establish(&url)?;
            run_postgres_migrations(&conn)?;
        }

        let result = panic::catch_unwind(move || test(&url));

        // drop all the tables
        let conn = PgConnection::establish(&base_url)?;
        let drop_tables_res: Result<(), Box<dyn Error>> = (|| {
            conn.batch_execute(&format!("DROP DATABASE {};", db_name))?;
            Ok(())
        })();

        (drop_tables_res, result)
    };

    match test_result {
        Ok(res) => drop_tables_res.and(res),
        Err(err) => {
            panic::resume_unwind(err);
        }
    }
}

fn db_name() -> String {
    static GLOBAL_THREAD_COUNT: AtomicUsize = AtomicUsize::new(1);

    let current_thread = std::thread::current();
    let thread_id = current_thread.name();
    // thread names during the unit test process are the path of the test being run, minus the
    // crate name.
    thread_id
        .and_then(|test_name| test_name.rsplit("::").next().map(String::from))
        .unwrap_or_else(|| {
            format!(
                "test_db_{}",
                GLOBAL_THREAD_COUNT.fetch_add(1, Ordering::SeqCst)
            )
        })
}
