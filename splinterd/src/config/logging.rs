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

use std::convert::From;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::ops::Deref;

use log::Level;

use super::error::ConfigError;
use super::toml::{TomlRawLogTarget, TomlUnnamedAppenderConfig, TomlUnnamedLoggerConfig};

const DEFAULT_LOGGING_PATTERN: &str = "[{d(%Y-%m-%d %H:%M:%S%.3f)}] T[{T}] {l} [{M}] {m}\n";
const DEFAULT_LOG_SIZE: u64 = 100_000_000;

#[derive(Clone, Debug)]
pub struct LogConfig {
    pub root: RootConfig,
    pub appenders: Vec<AppenderConfig>,
    pub loggers: Vec<LoggerConfig>,
}

#[derive(Clone, Debug)]
pub struct LoggerConfig {
    pub name: String,
    pub appenders: Option<Vec<String>>,
    pub level: Option<Level>,
}

#[derive(Clone, Debug)]
pub struct UnnamedLoggerConfig {
    pub appenders: Option<Vec<String>>,
    pub level: Option<Level>,
}

#[derive(Clone, Debug)]
pub struct RootConfig {
    pub appenders: Vec<String>,
    pub level: Level,
}

#[derive(Clone, Debug)]
pub struct AppenderConfig {
    pub name: String,
    pub encoder: LogEncoder,
    pub kind: LogTarget,
    pub level: Option<Level>,
}

#[derive(Clone, Debug)]
pub struct UnnamedAppenderConfig {
    pub encoder: LogEncoder,
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

#[derive(Clone, Debug)]
pub enum RawLogTarget {
    Stdout,
    Stderr,
    File,
    RollingFile,
}

#[derive(Clone, Debug)]
pub struct LogEncoder {
    value: String,
}

impl From<String> for LogEncoder {
    fn from(value: String) -> Self {
        LogEncoder { value }
    }
}

impl Default for LogEncoder {
    fn default() -> Self {
        Self {
            value: DEFAULT_LOGGING_PATTERN.to_string(),
        }
    }
}

impl Deref for LogEncoder {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
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
                if let Some(filename) = value.1.filename {
                    Ok(LogTarget::RollingFile {
                        filename,
                        size: value.1.size.unwrap_or(DEFAULT_LOG_SIZE),
                    })
                } else {
                    Err(ConfigError::MissingValue("filename".to_string()))
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
            encoder: unnamed
                .encoder
                .map_or_else(LogEncoder::default, |f| f.into()),
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
            level: unnamed.level.map(|v| v.into()),
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

impl TryFrom<TomlUnnamedLoggerConfig> for RootConfig {
    type Error = ConfigError;
    fn try_from(value: TomlUnnamedLoggerConfig) -> Result<Self, Self::Error> {
        match (value.appenders, value.level) {
            (Some(appenders), Some(level)) => Ok(Self {
                appenders,
                level: level.into(),
            }),
            (Some(appenders), None) => {
                let level = Level::Warn;
                Ok(Self { appenders, level })
            }
            (None, _) => Err(ConfigError::MissingValue("root.appenders".to_string())),
        }
    }
}
