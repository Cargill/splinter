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

use crate::error::CliError;

use super::{
    api::{RoleBuilder, RoleUpdateBuilder, SplinterRestClient, SplinterRestClientBuilder},
    create_cylinder_jwt_auth, print_table, Action, DEFAULT_SPLINTER_REST_API_URL,
    SPLINTER_REST_API_URL_ENV,
};

pub struct ListRolesAction;

impl Action for ListRolesAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let format = arg_matches
            .and_then(|args| args.value_of("format"))
            .unwrap_or("human");

        let client = new_client(&arg_matches)?;
        let roles = client.list_roles()?;

        let header = vec!["ID".to_string(), "DISPLAY NAME".to_string()];

        if format == "csv" {
            println!("{}", header.join(","));
            for role_res in roles {
                let role = role_res?;
                println!("{},{}", role.role_id, role.display_name);
            }
        } else {
            let mut rows = vec![header];
            for role_res in roles {
                let role = role_res?;
                rows.push(vec![role.role_id, role.display_name]);
            }
            print_table(rows);
        }

        Ok(())
    }
}

pub struct ShowRoleAction;

impl Action for ShowRoleAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let format = arg_matches
            .and_then(|args| args.value_of("format"))
            .unwrap_or("human");

        let role_id = arg_matches
            .and_then(|args| args.value_of("role_id"))
            .ok_or_else(|| CliError::ActionError("A role ID must be specified".into()))?;

        let role = new_client(&arg_matches)?.get_role(role_id)?;

        match format {
            "json" => println!(
                "\n {}",
                serde_json::to_string(&role).map_err(|err| CliError::ActionError(format!(
                    "Cannot format role into json: {}",
                    err
                )))?
            ),
            "yaml" => println!(
                "{}",
                serde_yaml::to_string(&role).map_err(|err| CliError::ActionError(format!(
                    "Cannot format role into yaml: {}",
                    err
                )))?
            ),
            _ => println!("{}", role),
        }

        Ok(())
    }
}

pub struct CreateRoleAction;

impl Action for CreateRoleAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let role_id = arg_matches
            .and_then(|args| args.value_of("role_id"))
            .ok_or_else(|| CliError::ActionError("A role must have an ID".into()))?;

        let display_name = arg_matches
            .and_then(|args| args.value_of("display_name"))
            .ok_or_else(|| CliError::ActionError("A role must have a display name".into()))?;

        let permissions = arg_matches
            .and_then(|args| args.values_of("permission"))
            .ok_or_else(|| {
                CliError::ActionError("A role must have at least one permission".into())
            })?
            .map(|s| s.to_owned())
            .collect();

        new_client(&arg_matches)?.create_role(
            RoleBuilder::default()
                .with_role_id(role_id.into())
                .with_display_name(display_name.into())
                .with_permissions(permissions)
                .build()?,
        )
    }
}

pub struct UpdateRoleAction;

impl Action for UpdateRoleAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let role_id = arg_matches
            .and_then(|args| args.value_of("role_id"))
            .ok_or_else(|| CliError::ActionError("A role id must be provided.".into()))?;

        let display_name = arg_matches
            .and_then(|args| args.value_of("display_name"))
            .map(|s| s.to_owned());

        let permissions_to_add = arg_matches
            .and_then(|args| args.values_of("add_permission"))
            .map(|vals| vals.map(|s| s.to_owned()).collect())
            .unwrap_or_else(Vec::new);

        let permissions_to_rm = arg_matches
            .and_then(|args| args.values_of("rm_permission"))
            .map(|vals| vals.map(|s| s.to_owned()).collect())
            .unwrap_or_else(Vec::new);

        let client = new_client(&arg_matches)?;

        let role = client.get_role(role_id)?;

        let mut permissions = role
            .permissions
            .into_iter()
            .chain(permissions_to_add.into_iter())
            .filter(|perm| !permissions_to_rm.contains(&perm))
            .collect::<Vec<_>>();

        permissions.sort();
        permissions.dedup();

        client.update_role(
            RoleUpdateBuilder::default()
                .with_role_id(role_id.into())
                .with_display_name(display_name)
                .with_permissions(Some(permissions))
                .build()?,
        )
    }
}

fn new_client(arg_matches: &Option<&ArgMatches<'_>>) -> Result<SplinterRestClient, CliError> {
    let url = arg_matches
        .and_then(|args| args.value_of("url"))
        .map(ToOwned::to_owned)
        .or_else(|| std::env::var(SPLINTER_REST_API_URL_ENV).ok())
        .unwrap_or_else(|| DEFAULT_SPLINTER_REST_API_URL.to_string());

    let key = arg_matches.and_then(|args| args.value_of("private_key_file"));

    SplinterRestClientBuilder::new()
        .with_url(url)
        .with_auth(create_cylinder_jwt_auth(key)?)
        .build()
}
