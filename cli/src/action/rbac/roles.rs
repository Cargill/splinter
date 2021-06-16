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

//! Actions to support the RBAC subcommands related to roles.

use std::collections::BTreeSet;

use clap::ArgMatches;

use crate::action::{
    api::{RoleBuilder, RoleUpdateBuilder, SplinterRestClient},
    print_table, Action,
};
use crate::error::CliError;

use super::new_client;

/// The action responsible for listing roles.
///
/// The specific args for this action:
///
/// * format: specifies the output format; one of "human" or "csv"
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

/// The action responsible for showing a specific role.
///
/// The specific args for this action:
///
/// * role_id: the specified role ID
/// * format: specifies the output format; one of "human", "json", or "yaml"
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

/// The action responsible for creating roles.
///
/// The specific args for this action:
///
/// * role_id: the specified role ID
/// * display_name: the role's display name
/// * permission: a permission granted by the resulting role; repeated
/// * dry_run: validate the inputs but do not submit the role
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

        let role = RoleBuilder::default()
            .with_role_id(role_id.into())
            .with_display_name(display_name.into())
            .with_permissions(permissions)
            .build()?;

        let client = new_client(&arg_matches)?;
        if !is_dry_run(&arg_matches) {
            client.create_role(role)
        } else if client.get_role(role_id)?.is_some() {
            Err(CliError::ActionError(format!(
                "A Role with ID {} already exists",
                role_id
            )))
        } else {
            Ok(())
        }
    }
}

/// The action responsible for updating a specific role.
///
/// The specific args for this action:
///
/// * role_id: the specified role ID
/// * display_name: the role's display name
/// * add_permission: a permission to add to the role; repeated
/// * rm_permission: a permission to remove from the role; repeated
/// * rm_all: remove all the currently granted permissions from the role
/// * force: applies the changes, even if a permission is added and removed
/// * dry_run: validate the inputs but do not submit the changes
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
            is_dry_run(&arg_matches),
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
    is_dry_run: bool,
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

    let updated_role = RoleUpdateBuilder::default()
        .with_role_id(role_id.into())
        .with_display_name(display_name)
        .with_permissions(Some(permissions))
        .build()?;

    if !is_dry_run {
        client.update_role(updated_role)
    } else {
        Ok(())
    }
}

/// The action responsible for deleting a specific role.
///
/// The specific args for this action:
///
/// * role_id: the specified role ID
pub struct DeleteRoleAction;

impl Action for DeleteRoleAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let role_id = arg_matches
            .and_then(|args| args.value_of("role_id"))
            .ok_or_else(|| CliError::ActionError("A role ID must be specified".into()))?;

        if !is_dry_run(&arg_matches) {
            new_client(&arg_matches)?.delete_role(role_id)
        } else {
            Ok(())
        }
    }
}

fn is_dry_run<'a>(arg_matches: &Option<&ArgMatches<'a>>) -> bool {
    arg_matches
        .map(|args| args.is_present("dry_run"))
        .unwrap_or(false)
}
