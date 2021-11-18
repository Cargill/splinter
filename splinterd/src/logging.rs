// Copyright 2018-2022 Cargill Incorporated
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
use std::fs::OpenOptions;
use std::path::Path;

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
use splinter::error::InternalError;

use crate::config::{
    AppenderConfig, Config as InternalConfig, LogConfig, LogEncoder, LogTarget, LoggerConfig,
    RootConfig,
};
use crate::error::UserError;

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

struct LoggerFactory {
    default: RootConfig,
}
impl LoggerFactory {
    fn new(default: RootConfig) -> Self {
        Self { default }
    }
    fn get_logger(&self, config: LoggerConfig) -> Logger {
        let level = config
            .level
            .map(|l| l.to_level_filter())
            .unwrap_or_else(|| self.default.level.to_level_filter());
        let appenders = config.appenders.unwrap_or_default();
        Logger::builder()
            .appenders(appenders)
            .build(config.name, level)
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

impl TryInto<Config> for LogConfig {
    type Error = ConfigErrors;
    fn try_into(self) -> Result<Config, Self::Error> {
        let factory = LoggerFactory::new(self.root.clone());
        let root = self.root.into();
        Config::builder()
            .appenders(
                self.appenders
                    .iter()
                    .filter_map(|ac| ac.to_owned().try_into().ok()),
            )
            .loggers(self.loggers.into_iter().map(|lc| factory.get_logger(lc)))
            .build(root)
    }
}

pub fn configure_logging(
    config: &InternalConfig,
    log_handle: &log4rs::Handle,
) -> Result<(), UserError> {
    let appenders = if let Some(appenders) = config.appenders() {
        let check_file_readability = |path: &Path| {
            OpenOptions::new()
                .write(true)
                .create(!path.exists())
                .open(path)
                .map(|_| ())
                .map_err(|err| UserError::IoError {
                    context: format!("logfile is not writeable: {}", path.display()),
                    source: Some(Box::new(err)),
                })
        };
        appenders
            .iter()
            .filter_map(AppenderConfig::get_filename)
            .try_for_each(|filename| {
                let path = Path::new(filename);
                if let Some(parent) = path.parent() {
                    if !parent.exists() {
                        Err(UserError::IoError {
                            context: format!(
                                "logfile directory does not exist: {}",
                                parent.display()
                            ),
                            source: None,
                        })
                    } else {
                        check_file_readability(path)
                    }
                } else {
                    check_file_readability(path)
                }
            })?;
        appenders
            .iter()
            .map(|a| {
                if a.name == "stdout" {
                    AppenderConfig {
                        level: Some(config.verbosity()),
                        name: a.name.to_owned(),
                        encoder: a.encoder.to_owned(),
                        kind: a.kind.to_owned(),
                    }
                } else {
                    a.to_owned()
                }
            })
            .collect()
    } else {
        vec![]
    };
    let loggers = if let Some(loggers) = config.loggers() {
        loggers
    } else {
        vec![]
    };
    let log_config = LogConfig {
        root: config.root_logger().to_owned(),
        appenders,
        loggers,
    };
    match log_config.try_into() {
        Ok(log_config) => {
            log_handle.set_config(log_config);
            Ok(())
        }
        Err(e) => Err(UserError::InternalError(InternalError::from_source(
            Box::new(e),
        ))),
    }
}

pub fn default_log_settings() -> Config {
    let default_config: LogConfig = LogConfig {
        root: RootConfig {
            appenders: vec![String::from("default")],
            level: log::Level::Debug,
        },
        appenders: vec![AppenderConfig {
            name: String::from("default"),
            encoder: LogEncoder::default(),
            kind: LogTarget::Stdout,
            level: None,
        }],
        loggers: vec![],
    };
    if let Ok(log_config) = default_config.try_into() {
        log_config
    } else {
        unreachable!()
    }
}
