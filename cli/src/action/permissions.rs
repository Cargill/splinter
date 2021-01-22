// Copyright 2018-2020 Cargill Incorporated
// Copyright 2018 Intel Corporation
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

use clap::ArgMatches;

use crate::error::CliError;

use super::{
    api::SplinterRestClientBuilder, create_cylinder_jwt_auth, print_table, Action,
    DEFAULT_SPLINTER_REST_API_URL, SPLINTER_REST_API_URL_ENV,
};

pub struct ListAction;

impl Action for ListAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let format = arg_matches
            .and_then(|args| args.value_of("format"))
            .unwrap_or("human");
        let url = arg_matches
            .and_then(|args| args.value_of("url"))
            .map(ToOwned::to_owned)
            .or_else(|| std::env::var(SPLINTER_REST_API_URL_ENV).ok())
            .unwrap_or_else(|| DEFAULT_SPLINTER_REST_API_URL.to_string());
        let key = arg_matches.and_then(|args| args.value_of("private_key_file"));

        let permissions = SplinterRestClientBuilder::new()
            .with_url(url)
            .with_auth(create_cylinder_jwt_auth(key)?)
            .build()?
            .list_permissions()?;

        let data = std::iter::once(vec![
            "ID".to_string(),
            "NAME".to_string(),
            "DESCRIPTION".to_string(),
        ])
        .chain(permissions.into_iter().map(|perm| {
            vec![
                perm.permission_id,
                perm.permission_display_name,
                perm.permission_description,
            ]
        }));

        match format {
            "csv" => {
                for row in data {
                    println!("{}", row.join(","))
                }
            }
            "json" => println!(
                "\n {}",
                serde_json::to_string_pretty(&data.collect::<Vec<_>>()).map_err(|err| {
                    CliError::ActionError(format!("Cannot format permissions into json: {}", err))
                })?
            ),
            _ => print_table(data.collect()),
        }

        Ok(())
    }
}
