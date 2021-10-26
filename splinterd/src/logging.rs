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

use std::convert::{From, Into, TryInto};

use log::Level;
use log4rs::{
    append::{
        console::{ConsoleAppender, Target},
        file::FileAppender,
        rolling_file::{
            policy::{
                compound::{
                    roll::delete::DeleteRoller, trigger::size::SizeTrigger, CompoundPolicy,
                },
                Policy,
            },
            RollingFileAppender,
        },
        Append,
    },
    config::{runtime::ConfigErrors, Appender, Logger, Root},
    encode::{pattern::PatternEncoder, Encode},
    filter::threshold::ThresholdFilter,
    Config,
};

use crate::config::{AppenderConfig, LogConfig, LogTarget, LoggerConfig, RootConfig};

impl TryInto<Appender> for AppenderConfig {
    type Error = std::io::Error;
    fn try_into(self) -> Result<Appender, Self::Error> {
        let encoder: Box<dyn Encode> = Box::new(PatternEncoder::new(&self.encoder));
        let boxed: Box<dyn Append> = match &self.kind {
            LogTarget::Stdout => Box::new(
                ConsoleAppender::builder()
                    .encoder(encoder)
                    .target(Target::Stdout)
                    .build(),
            ),
            LogTarget::Stderr => Box::new(
                ConsoleAppender::builder()
                    .encoder(encoder)
                    .target(Target::Stderr)
                    .build(),
            ),
            LogTarget::File(path) => {
                Box::new(FileAppender::builder().encoder(encoder).build(path)?)
            }
            LogTarget::RollingFile { filename, size } => {
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
        let mut builder = Appender::builder();
        if let Some(level) = self.level {
            builder = builder.filter(Box::new(ThresholdFilter::new(level.to_level_filter())))
        }
        Ok(builder.build(&self.name, boxed))
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
