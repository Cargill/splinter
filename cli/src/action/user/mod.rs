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

mod api;

use clap::ArgMatches;
use cylinder::Signer;

use crate::error::CliError;
use crate::signing::{create_cylinder_jwt_auth, load_signer};

use super::api::SplinterRestClientBuilder;
use super::{print_table, Action, DEFAULT_SPLINTER_REST_API_URL, SPLINTER_REST_API_URL_ENV};
use api::{ClientBiomeUser, ClientOAuthUser};

pub struct ListSplinterUsersAction;

impl Action for ListSplinterUsersAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let args = arg_matches.ok_or(CliError::RequiresArgs)?;

        let format = args.value_of("format").unwrap_or("human");
        let signer = load_signer(args.value_of("private_key_file"))?;
        let url = args
            .value_of("url")
            .map(ToOwned::to_owned)
            .or_else(|| std::env::var(SPLINTER_REST_API_URL_ENV).ok())
            .unwrap_or_else(|| DEFAULT_SPLINTER_REST_API_URL.to_string());

        display_splinter_users(&url, format, signer)
    }
}

fn display_splinter_users(
    url: &str,
    format: &str,
    signer: Box<dyn Signer>,
) -> Result<(), CliError> {
    let client = SplinterRestClientBuilder::new()
        .with_url(url.to_string())
        .with_auth(create_cylinder_jwt_auth(signer.clone())?)
        .build()?;

    let biome_users = client
        .list_biome_users()?
        .into_iter()
        .map(ClientSplinterUser::from);

    let biome_oauth_users = client
        .list_oauth_users()?
        .data
        .into_iter()
        .map(ClientSplinterUser::from);

    let mut data = vec![
        // headers
        vec!["ID".to_string(), "USERNAME".to_string(), "TYPE".to_string()],
    ];
    let users = biome_users.into_iter().chain(biome_oauth_users.into_iter());
    users.into_iter().for_each(|user| match user {
        ClientSplinterUser::Biome(user) => {
            data.push(vec![user.user_id, user.username, "Biome".to_string()])
        }
        ClientSplinterUser::OAuth(user) => {
            data.push(vec![user.user_id, user.subject, "OAuth".to_string()])
        }
    });

    if format == "csv" {
        for row in data {
            println!("{}", row.join(","));
        }
    } else {
        print_table(data);
    }

    Ok(())
}

/// Representation of the users that may be returned by Splinter.
enum ClientSplinterUser {
    Biome(ClientBiomeUser),
    OAuth(ClientOAuthUser),
}

impl From<ClientBiomeUser> for ClientSplinterUser {
    fn from(client_user: ClientBiomeUser) -> Self {
        ClientSplinterUser::Biome(client_user)
    }
}

impl From<ClientOAuthUser> for ClientSplinterUser {
    fn from(client_user: ClientOAuthUser) -> Self {
        ClientSplinterUser::OAuth(client_user)
    }
}
