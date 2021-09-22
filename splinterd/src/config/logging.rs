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
use serde::Deserialize;
use std::convert::From;
use std::convert::TryFrom;

use super::bytes::ByteSize;
use super::error::ConfigError;

pub const DEFAULT_LOGGING_PATTERN: &str = "[{d(%Y-%m-%d %H:%M:%S%.3f)}] T[{T}] {l} [{M}] {m}\n";

fn default_pattern() -> String {
    String::from(DEFAULT_LOGGING_PATTERN)
}

#[derive(Clone, Debug)]
pub struct LogConfig {
    pub root: RootConfig,
    pub appenders: Vec<AppenderConfig>,
    pub loggers: Vec<LoggerConfig>,
}

#[derive(Clone, Debug)]
pub struct LoggerConfig {
    pub name: String,
    pub appenders: Vec<String>,
    pub level: Level,
}

#[derive(Deserialize, Clone, Debug)]
pub struct UnnamedLoggerConfig {
    appenders: Vec<String>,
    #[serde(alias = "filter")]
    level: Level,
}

#[derive(Deserialize, Clone, Debug)]
pub struct RootConfig {
    pub appenders: Vec<String>,
    pub level: Level,
}

#[derive(Clone, Debug)]
pub struct AppenderConfig {
    pub name: String,
    pub encoder: String,
    pub kind: LogTarget,
}

#[derive(Deserialize, Clone, Debug)]
pub struct UnnamedAppenderConfig {
    #[serde(default = "default_pattern")]
    #[serde(alias = "pattern")]
    pub encoder: String,
    pub kind: RawLogTarget,
    pub filename: Option<String>,
    pub size: Option<ByteSize>,
}

#[derive(Clone, Debug)]
pub enum LogTarget {
    Stdout,
    Stderr,
    File(String),
    RollingFile { filename: String, size: u64 },
}

#[derive(Deserialize, Clone, Debug)]
pub enum RawLogTarget {
    #[serde(alias = "stdout")]
    Stdout,
    #[serde(alias = "stderr")]
    Stderr,
    #[serde(alias = "file")]
    File,
    #[serde(alias = "rolling_file")]
    RollingFile,
}

impl TryFrom<(String, UnnamedAppenderConfig)> for AppenderConfig {
    type Error = ConfigError;
    fn try_from(value: (String, UnnamedAppenderConfig)) -> Result<Self, Self::Error> {
        use crate::config::RawLogTarget::*;
        let kind = match value.1.kind {
            Stdout => Ok(LogTarget::Stdout),
            Stderr => Ok(LogTarget::Stderr),
            File => {
                if let Some(filename) = value.1.filename {
                    Ok(LogTarget::File(filename))
                } else {
                    Err(ConfigError::MissingValue("filename".to_string()))
                }
            }
            RollingFile => {
                if let (Some(filename), Some(size)) = (value.1.filename, value.1.size) {
                    let size = size.get_mem_size();
                    Ok(LogTarget::RollingFile { filename, size })
                } else {
                    Err(ConfigError::MissingValue("filename|size".to_string()))
                }
            }
        }?;
        Ok(AppenderConfig {
            name: value.0,
            encoder: value.1.encoder,
            kind,
        })
    }
}
impl From<(String, UnnamedLoggerConfig)> for LoggerConfig {
    fn from(pair: (String, UnnamedLoggerConfig)) -> Self {
        Self {
            name: pair.0,
            appenders: pair.1.appenders,
            level: pair.1.level,
        }
    }
}

impl From<UnnamedLoggerConfig> for RootConfig {
    fn from(un_named: UnnamedLoggerConfig) -> Self {
        Self {
            appenders: un_named.appenders,
            level: un_named.level,
        }
    }
}
