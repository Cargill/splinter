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

use clap::ArgMatches;

use crate::error::CliError;
use crate::template::CircuitTemplate;

use super::Action;

pub struct ListCircuitTemplates;

impl Action for ListCircuitTemplates {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        // Collect list of template file stems and full paths to the associated file stem
        let templates = CircuitTemplate::list_available_templates()?;

        let format = arg_matches
            .and_then(|args| args.value_of("format"))
            .unwrap_or("human");

        if format == "csv" {
            print!("TEMPLATE,PATH,");
            for (stem, path) in templates.iter() {
                print!("{},{},", stem, path.display());
            }
            println!();
        } else {
            // Initialize the maximum column length for the first column in the template table,
            // currently set to 8 as this is the length of the `TEMPLATE` header.
            let mut max_length = 8;

            // Find the max lengths of the initial column
            for (stem, _) in templates.iter() {
                if stem.len() > max_length {
                    max_length = stem.len()
                }
            }
            // Print headers for the template table to be displayed
            let header_string = format!("{}{} {} ", "TEMPLATE", " ".repeat(max_length - 8), "PATH");
            println!("{}", header_string);
            // Iterate through the list of template file stems and file paths to be displayed
            for (stem, path) in templates.iter() {
                let row = format!(
                    "{}{} {} ",
                    &stem,
                    " ".repeat(max_length - stem.len()),
                    path.display()
                );
                println!("{}", row);
            }
        }

        Ok(())
    }
}

pub struct ShowCircuitTemplate;

impl Action for ShowCircuitTemplate {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let args = arg_matches.ok_or_else(|| CliError::RequiresArgs)?;
        let template_name = match args.value_of("name") {
            Some(name) => name,
            None => return Err(CliError::ActionError("Name is required".into())),
        };

        let template = CircuitTemplate::load_raw(template_name)?;

        println!("{}", template);

        Ok(())
    }
}

pub struct ListCircuitTemplateArguments;

impl Action for ListCircuitTemplateArguments {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let args = arg_matches.ok_or_else(|| CliError::RequiresArgs)?;
        let template_name = match args.value_of("name") {
            Some(name) => name,
            None => return Err(CliError::ActionError("Name is required".into())),
        };

        let template = CircuitTemplate::load(template_name)?;

        let arguments = template.arguments();
        for argument in arguments {
            println!("\nname: {}", argument.name());
            println!("required: {}", argument.required());
            println!(
                "default_value: {}",
                argument.default_value().unwrap_or(&"Not set".to_string())
            );
            println!(
                "description: {}",
                argument.description().unwrap_or(&"Not set".to_string())
            );
        }

        Ok(())
    }
}
