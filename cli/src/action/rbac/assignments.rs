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

use clap::ArgMatches;

use crate::action::{api::{AssignmentBuilder, Identity}, print_table, Action};
use crate::error::CliError;

use super::new_client;

pub struct ListAssignmentsAction;

impl Action for ListAssignmentsAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let format = arg_matches
            .and_then(|args| args.value_of("format"))
            .unwrap_or("human");

        let client = new_client(&arg_matches)?;

        let mut assignments = client
            .list_assignments()?
            .map(|res| {
                res.map(|assignment| {
                    let (id, id_type) = match assignment.identity {
                        Identity::Key(key) => (key, String::from("key")),
                        Identity::User(user_id) => (user_id, String::from("user")),
                    };

                    vec![id, id_type, assignment.roles.len().to_string()]
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let header = vec![
            "IDENTITY".to_string(),
            "TYPE".to_string(),
            "ROLES".to_string(),
        ];
        if format == "csv" {
            println!("{}", header.join(","));
            for row in assignments {
                println!("{}", row.join(","));
            }
        } else {
            let mut rows = vec![header];
            rows.append(&mut assignments);
            print_table(rows);
        }

        Ok(())
    }
}

pub struct CreateAssignmentAction;

impl Action for CreateAssignmentAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let identity = get_identity_arg(&arg_matches)?;

        let roles = arg_matches
            .and_then(|args| args.values_of("role"))
            .ok_or_else(|| {
                CliError::ActionError("At least one role must be assigned".into())
            })?
            .map(|s| s.to_owned())
            .collect();

        new_client(&arg_matches)?.create_assignment(
            AssignmentBuilder::default()
            .with_identity(identity)
            .with_roles(roles)
            .build()?,
        )
    }
}

fn get_identity_arg<'a>(arg_matches: &Option<&ArgMatches<'a>>) -> Result<Identity, CliError> {
    if let Some(key) = arg_matches
        .and_then(|args| args.value_of("id_key"))
        .map(|s| s.to_string())
    {
        return Ok(Identity::Key(key));
    }

    if let Some(user_id) = arg_matches
        .and_then(|args| args.value_of("id_user"))
        .map(|s| s.to_string())
    {
        return Ok(Identity::User(user_id));
    }

    Err(CliError::ActionError(
        "Must specify either key or user identity".into(),
    ))
}
