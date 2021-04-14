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

use std::collections::BTreeSet;

use clap::ArgMatches;

use crate::action::{
    api::{RoleBuilder, RoleUpdateBuilder, SplinterRestClient},
    print_table, Action,
};
use crate::error::CliError;

use super::new_client;

pub struct ListRolesAction;

impl Action for ListRolesAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let format = arg_matches
            .and_then(|args| args.value_of("format"))
            .unwrap_or("human");

        let client = new_client(&arg_matches)?;
        let roles = client.list_roles()?;

        let header = vec!["ID".to_string(), "NAME".to_string()];

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

        let role = new_client(&arg_matches)?
            .get_role(role_id)?
            .ok_or_else(|| CliError::ActionError(format!("Role {} does not exist", role_id)))?;

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

        let rm_all = arg_matches
            .map(|args| args.is_present("rm_all"))
            .unwrap_or(false);
        let permission_removal = if rm_all {
            PermissionRemoval::RemoveAll
        } else {
            PermissionRemoval::Remove(
                arg_matches
                    .and_then(|args| args.values_of("rm_permission"))
                    .map(|vals| vals.map(|s| s.to_owned()).collect())
                    .unwrap_or_else(Vec::new),
            )
        };

        let force = arg_matches
            .map(|args| args.is_present("force"))
            .unwrap_or(false);

        update_role(
            new_client(&arg_matches)?,
            role_id,
            display_name,
            permissions_to_add,
            permission_removal,
            force,
        )
    }
}

enum PermissionRemoval {
    RemoveAll,
    Remove(Vec<String>),
}

fn update_role(
    client: SplinterRestClient,
    role_id: &str,
    display_name: Option<String>,
    permissions_to_add: Vec<String>,
    permission_removal: PermissionRemoval,
    force: bool,
) -> Result<(), CliError> {
    let role = client
        .get_role(role_id)?
        .ok_or_else(|| CliError::ActionError(format!("Role {} does not exist", role_id)))?;

    let permissions = match permission_removal {
        PermissionRemoval::RemoveAll => {
            println!("Removing permissions {}", role.permissions.join(", "));
            permissions_to_add
        }
        PermissionRemoval::Remove(permissions_to_rm) => {
            let mut permissions_to_add = permissions_to_add.into_iter().collect::<BTreeSet<_>>();
            let mut permissions_to_rm = permissions_to_rm.into_iter().collect::<BTreeSet<_>>();

            if !force && permissions_to_add.intersection(&permissions_to_rm).count() > 0 {
                return Err(CliError::ActionError(format!(
                    "Cannot add and remove the same permissions: {}",
                    permissions_to_add
                        .intersection(&permissions_to_rm)
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )));
            }

            let mut current_permissions = role
                .permissions
                .into_iter()
                .filter(|perm| !permissions_to_rm.remove(perm))
                .collect::<BTreeSet<_>>();

            current_permissions.append(&mut permissions_to_add);

            let permissions = current_permissions.into_iter().collect::<Vec<_>>();

            if !force && !permissions_to_rm.is_empty() {
                return Err(CliError::ActionError(format!(
                    "Cannot remove permissions that do not belong to the role: {}",
                    permissions_to_rm
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )));
            }

            permissions
        }
    };

    client.update_role(
        RoleUpdateBuilder::default()
            .with_role_id(role_id.into())
            .with_display_name(display_name)
            .with_permissions(Some(permissions))
            .build()?,
    )
}

pub struct DeleteRoleAction;

impl Action for DeleteRoleAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let role_id = arg_matches
            .and_then(|args| args.value_of("role_id"))
            .ok_or_else(|| CliError::ActionError("A role ID must be specified".into()))?;

        new_client(&arg_matches)?.delete_role(role_id)
    }
}
