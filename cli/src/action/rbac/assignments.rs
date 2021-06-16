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

//! Actions to support the RBAC subcommands related to authorizing identities.

use std::collections::BTreeSet;

use clap::ArgMatches;

use crate::action::{
    api::{Assignment, AssignmentBuilder, AssignmentUpdateBuilder, Identity, SplinterRestClient},
    print_table, Action,
};
use crate::error::CliError;

use super::new_client;

/// The action responsible for listing authorized identities.
///
/// The specific args for this action:
///
/// * format: specifies the output format; one of "human" or "csv"
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
                    let (id, id_type) = assignment.identity.parts();
                    vec![
                        id.to_string(),
                        id_type.to_string(),
                        assignment.roles.len().to_string(),
                    ]
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

/// The action responsible for creating authorized identities.
///
/// The specific args for this action:
///
/// * id_key: an identifier of type key; a public key
/// * id_user: an identifier of type user; a user ID
/// * role: a role to add to the assignment; repeated
pub struct CreateAssignmentAction;

impl Action for CreateAssignmentAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let identity = get_identity_arg(&arg_matches)?;

        let roles = arg_matches
            .and_then(|args| args.values_of("role"))
            .ok_or_else(|| CliError::ActionError("At least one role must be assigned".into()))?
            .map(|s| s.to_owned())
            .collect();

        let assignment = AssignmentBuilder::default()
            .with_identity(identity.clone())
            .with_roles(roles)
            .build()?;
        let client = new_client(&arg_matches)?;
        if !is_dry_run(&arg_matches) {
            client.create_assignment(assignment)
        } else if client.get_assignment(&identity)?.is_some() {
            let (id_value, id_type) = identity.parts();
            Err(CliError::ActionError(format!(
                "An assignment for {} {} already exists",
                id_type, id_value
            )))
        } else {
            Ok(())
        }
    }
}

/// The action responsible for showing a specific authorized identity.
///
/// The specific args for this action:
///
/// * id_key: an identifier of type key; a public key
/// * id_user: an identifier of type user; a user ID
/// * format: specifies the output format; one of "human", "json", or "yaml"
pub struct ShowAssignmentAction;

impl Action for ShowAssignmentAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let format = arg_matches
            .and_then(|args| args.value_of("format"))
            .unwrap_or("human");

        let identity = get_identity_arg(&arg_matches)?;

        let assignment = new_client(&arg_matches)?
            .get_assignment(&identity)?
            .ok_or_else(|| {
                let (id_value, id_type) = identity.parts();
                CliError::ActionError(format!(
                    "Authorized identity {} {} does not exist",
                    id_type, id_value,
                ))
            })?;

        match format {
            "json" => println!(
                "\n {}",
                serde_json::to_string(&assignment).map_err(|err| CliError::ActionError(
                    format!("Cannot format assignment into json: {}", err)
                ))?
            ),
            "yaml" => println!(
                "{}",
                serde_yaml::to_string(&assignment).map_err(|err| CliError::ActionError(
                    format!("Cannot format assignment into yaml: {}", err)
                ))?
            ),
            _ => display_human_readable(&assignment),
        }

        Ok(())
    }
}

/// The action responsible for updating a specific authorized identity.
///
/// The specific args for this action:
///
/// * id_key: an identifier of type key; a public key
/// * id_user: an identifier of type user; a user ID
/// * add_role: a role to add to the assignment; repeated
/// * rm_role: a role to remove from the assignment; repeated
/// * rm_all: remove all the currently assigned roles
/// * dry_run: validate the inputs but do not submit the changes
/// * force: applies the changes, even if a role is added and removed
pub struct UpdateAssignmentAction;

impl Action for UpdateAssignmentAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let identity = get_identity_arg(&arg_matches)?;

        let force = arg_matches
            .map(|args| args.is_present("force"))
            .unwrap_or(false);

        let dry_run = is_dry_run(&arg_matches);

        let roles_to_add = arg_matches
            .and_then(|args| args.values_of("add_role"))
            .map(|vals| vals.map(|s| s.to_owned()).collect())
            .unwrap_or_else(Vec::new);

        let rm_all = arg_matches
            .map(|args| args.is_present("rm_all"))
            .unwrap_or(false);
        let role_removal = if rm_all {
            RoleRemoval::RemoveAll
        } else {
            RoleRemoval::Remove(
                arg_matches
                    .and_then(|args| args.values_of("rm_role"))
                    .map(|vals| vals.map(|s| s.to_owned()).collect())
                    .unwrap_or_else(Vec::new),
            )
        };

        update_assignment(
            new_client(&arg_matches)?,
            identity,
            roles_to_add,
            role_removal,
            force,
            dry_run,
        )
    }
}

enum RoleRemoval {
    RemoveAll,
    Remove(Vec<String>),
}

fn update_assignment(
    client: SplinterRestClient,
    identity: Identity,
    roles_to_add: Vec<String>,
    role_removal: RoleRemoval,
    force: bool,
    is_dry_run: bool,
) -> Result<(), CliError> {
    let assignment = client.get_assignment(&identity)?.ok_or_else(|| {
        let (id_value, id_type) = identity.parts();
        CliError::ActionError(format!(
            "Authorized identity {} {} does not exist",
            id_type, id_value,
        ))
    })?;

    let roles = match role_removal {
        RoleRemoval::RemoveAll => {
            println!("Removing roles {}", assignment.roles.join(", "));
            roles_to_add
        }
        RoleRemoval::Remove(roles_to_rm) => {
            let mut roles_to_add = roles_to_add.into_iter().collect::<BTreeSet<_>>();
            let mut roles_to_rm = roles_to_rm.into_iter().collect::<BTreeSet<_>>();

            if !force && roles_to_add.intersection(&roles_to_rm).count() > 0 {
                return Err(CliError::ActionError(format!(
                    "Cannot add and remove the same roles: {}",
                    roles_to_add
                        .intersection(&roles_to_rm)
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )));
            }

            let mut current_roles = assignment
                .roles
                .into_iter()
                .filter(|perm| !roles_to_rm.remove(perm))
                .collect::<BTreeSet<_>>();

            current_roles.append(&mut roles_to_add);

            let roles = current_roles.into_iter().collect::<Vec<_>>();

            if !force && !roles_to_rm.is_empty() {
                return Err(CliError::ActionError(format!(
                    "Cannot remove roles that do not belong to the assignment: {}",
                    roles_to_rm
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )));
            }

            roles
        }
    };

    let updated_assignment = AssignmentUpdateBuilder::default()
        .with_identity(identity)
        .with_roles(Some(roles))
        .build()?;

    if !is_dry_run {
        client.update_assignment(updated_assignment)
    } else {
        Ok(())
    }
}

/// The action responsible for deleting a specific authorized identity.
///
/// The specific args for this action:
///
/// * id_key: an identifier of type key; a public key
/// * id_user: an identifier of type user; a user ID
/// * dry_run: validate the inputs but do not submit the changes
pub struct DeleteAssignmentAction;

impl Action for DeleteAssignmentAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let identity = get_identity_arg(&arg_matches)?;
        if !is_dry_run(&arg_matches) {
            new_client(&arg_matches)?.delete_assignment(&identity)
        } else {
            Ok(())
        }
    }
}

fn display_human_readable(assignment: &Assignment) {
    let (id, id_type) = assignment.identity.parts();
    println!("ID: {}", id);
    println!("    Type: {}", id_type);
    println!("    Roles:");
    for role in &assignment.roles {
        println!("        {}", role);
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

fn is_dry_run<'a>(arg_matches: &Option<&ArgMatches<'a>>) -> bool {
    arg_matches
        .map(|args| args.is_present("dry_run"))
        .unwrap_or(false)
}
