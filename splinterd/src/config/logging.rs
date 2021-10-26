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

use std::convert::From;
use std::convert::TryFrom;
use std::convert::TryInto;

use log::Level;
use serde::Deserialize;

use super::error::ConfigError;
use super::toml::TomlRawLogTarget;
use super::toml::TomlUnnamedAppenderConfig;
use super::toml::TomlUnnamedLoggerConfig;

pub const DEFAULT_LOGGING_PATTERN: &str = "[{d(%Y-%m-%d %H:%M:%S%.3f)}] T[{T}] {l} [{M}] {m}\n";

pub(super) fn default_pattern() -> String {
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
    pub appenders: Vec<String>,
    #[serde(alias = "filter")]
    pub level: Level,
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
    pub level: Option<Level>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct UnnamedAppenderConfig {
    #[serde(default = "default_pattern")]
    #[serde(alias = "pattern")]
    pub encoder: String,
    pub kind: RawLogTarget,
    pub filename: Option<String>,
    pub size: Option<u64>,
    pub level: Option<Level>,
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

impl AppenderConfig {
    pub fn get_filename(&self) -> Option<&str> {
        match &self.kind {
            LogTarget::File(file) => Some(file),
            LogTarget::RollingFile {
                filename: file,
                size: _,
            } => Some(file),
            _ => None,
        }
    }
}

impl From<TomlRawLogTarget> for RawLogTarget {
    fn from(unnamed: TomlRawLogTarget) -> Self {
        match unnamed {
            TomlRawLogTarget::File => RawLogTarget::File,
            TomlRawLogTarget::Stdout => RawLogTarget::Stdout,
            TomlRawLogTarget::Stderr => RawLogTarget::Stderr,
            TomlRawLogTarget::RollingFile => RawLogTarget::RollingFile,
        }
    }
}

impl TryFrom<(String, UnnamedAppenderConfig)> for AppenderConfig {
    type Error = ConfigError;
    fn try_from(value: (String, UnnamedAppenderConfig)) -> Result<Self, Self::Error> {
        let kind = match value.1.kind {
            RawLogTarget::Stdout => Ok(LogTarget::Stdout),
            RawLogTarget::Stderr => Ok(LogTarget::Stderr),
            RawLogTarget::File => {
                if let Some(filename) = value.1.filename {
                    Ok(LogTarget::File(filename))
                } else {
                    Err(ConfigError::MissingValue("filename".to_string()))
                }
            }
            RawLogTarget::RollingFile => {
                if let (Some(filename), Some(size)) = (value.1.filename, value.1.size) {
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
            level: value.1.level,
        })
    }
}

impl TryFrom<(String, TomlUnnamedAppenderConfig)> for AppenderConfig {
    type Error = <AppenderConfig as TryFrom<(String, UnnamedAppenderConfig)>>::Error;
    fn try_from(value: (String, TomlUnnamedAppenderConfig)) -> Result<Self, Self::Error> {
        let unnamed: UnnamedAppenderConfig = value.1.into();
        (value.0, unnamed).try_into()
    }
}

impl From<TomlUnnamedAppenderConfig> for UnnamedAppenderConfig {
    fn from(unnamed: TomlUnnamedAppenderConfig) -> Self {
        Self {
            encoder: unnamed.encoder,
            kind: unnamed.kind.into(),
            filename: unnamed.filename,
            size: unnamed.size.map(|s| s.into()),
            level: unnamed.level.map(|l| l.into()),
        }
    }
}

impl From<TomlUnnamedLoggerConfig> for UnnamedLoggerConfig {
    fn from(unnamed: TomlUnnamedLoggerConfig) -> Self {
        Self {
            appenders: unnamed.appenders,
            level: unnamed.level.into(),
        }
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

impl From<TomlUnnamedLoggerConfig> for RootConfig {
    fn from(unnamed: TomlUnnamedLoggerConfig) -> Self {
        Self {
            appenders: unnamed.appenders,
            level: unnamed.level.into(),
        }
    }
}
