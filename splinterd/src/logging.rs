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

use log::Level;
use log4rs::append::console::ConsoleAppender;
use log4rs::append::console::Target;
use log4rs::append::file::FileAppender;
use log4rs::append::rolling_file::policy::compound::roll::delete::DeleteRoller;
use log4rs::append::rolling_file::policy::compound::trigger::size::SizeTrigger;
use log4rs::append::rolling_file::policy::compound::CompoundPolicy;
use log4rs::append::rolling_file::policy::Policy;
use log4rs::append::rolling_file::RollingFileAppender;
use log4rs::append::Append;
use log4rs::config::runtime::ConfigErrors;
use log4rs::config::Appender;
use log4rs::config::Logger;
use log4rs::config::Root;
use log4rs::encode::pattern::PatternEncoder;
use log4rs::encode::Encode;
use log4rs::Config;
use std::convert::{From, Into, TryInto};

use crate::config::{AppenderConfig, LogConfig, LoggerConfig, RootConfig};

impl TryInto<Appender> for AppenderConfig {
    type Error = std::io::Error;
    fn try_into(self) -> Result<Appender, Self::Error> {
        use crate::config::LogTarget::*;
        let encoder: Box<dyn Encode> = Box::new(PatternEncoder::new(&self.encoder));
        let boxed: Box<dyn Append> = match &self.kind {
            Stdout => Box::new(
                ConsoleAppender::builder()
                    .encoder(encoder)
                    .target(Target::Stdout)
                    .build(),
            ),
            Stderr => Box::new(
                ConsoleAppender::builder()
                    .encoder(encoder)
                    .target(Target::Stderr)
                    .build(),
            ),
            File(path) => Box::new(FileAppender::builder().encoder(encoder).build(path)?),
            RollingFile { filename, size } => {
                let trigger = Box::new(SizeTrigger::new(*size));
                let roll = Box::new(DeleteRoller::new());
                let policy: Box<dyn Policy> = Box::new(CompoundPolicy::new(trigger, roll));

                Box::new(
                    RollingFileAppender::builder()
                        .encoder(encoder)
                        .build(filename, policy)?,
                )
            }
        };
        Ok(Appender::builder().build(&self.name, boxed))
    }
}

impl From<LoggerConfig> for Logger {
    fn from(logger_config: LoggerConfig) -> Self {
        let level = logger_config.level.to_level_filter();
        Logger::builder()
            .appenders(logger_config.appenders.clone())
            .build(&logger_config.name, level)
    }
}

impl From<RootConfig> for Root {
    fn from(root_config: RootConfig) -> Self {
        let level = root_config.level.to_level_filter();
        Root::builder()
            .appenders(root_config.appenders)
            .build(level)
    }
}

impl RootConfig {
    fn set_level(self, level: Level) -> Self {
        Self { level, ..self }
    }
}

impl TryInto<Config> for LogConfig {
    type Error = ConfigErrors;
    fn try_into(self) -> Result<Config, Self::Error> {
        let root = self.root.into();
        Config::builder()
            .appenders(
                self.appenders
                    .iter()
                    .filter_map(|ac| ac.to_owned().try_into().ok()),
            )
            .loggers(self.loggers.iter().map(|lc| lc.to_owned().into()))
            .build(root)
    }
}

impl LogConfig {
    pub fn set_root_level(self, level: Level) -> Self {
        Self {
            root: self.root.set_level(level),
            ..self
        }
    }
}
