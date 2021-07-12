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

pub const DEFAULT_PATTERN: &str = "[{d(%Y-%m-%d %H:%M:%S%.3f)}] T[{T}] {l} [{M}] {m}\n";

#[derive(Clone, Debug)]
pub struct LogConfig {
    pub root: RootConfig,
    pub appenders: Vec<AppenderConfig>,
    pub loggers: Vec<LoggerConfig>,
}

#[derive(Clone, Debug)]
pub struct LoggerConfig {
    name: String,
    appenders: Vec<String>,
    filter: Level,
}

#[derive(Clone, Debug)]
pub struct RootConfig {
    pub appenders: Vec<String>,
    pub filter: Level,
}

#[derive(Clone, Debug)]
pub struct AppenderConfig {
    pub name: String,
    pub encoder: String,
    pub kind: LogTarget,
}

#[derive(Clone, Debug)]
pub enum LogTarget {
    Stdout,
    Stderr,
    File(String),
    RollingFile(String),
}
