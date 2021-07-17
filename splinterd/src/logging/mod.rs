use log::Level;
use serde::Deserialize;

mod log4rs;
pub const DEFAULT_PATTERN: &str = "[{d(%Y-%m-%d %H:%M:%S%.3f)}] T[{T}] {l} [{M}] {m}\n";

#[derive(Deserialize, Clone, Debug)]
pub struct LogConfig {
    pub root: RootConfig,
    pub appenders: Vec<AppenderConfig>,
    pub loggers: Vec<LoggerConfig>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LoggerConfig {
    name: String,
    appenders: Vec<String>,
    level: Level,
}

#[derive(Deserialize, Clone, Debug)]
pub struct RootConfig {
    pub appenders: Vec<String>,
    pub level: Level,
}

#[derive(Deserialize, Clone, Debug)]
pub struct AppenderConfig {
    pub name: String,
    #[serde(default = "default_pattern")]
    pub encoder: String,
    pub target: LogTarget,
}

#[derive(Deserialize, Clone, Debug)]
pub enum LogTarget {
    #[serde(alias = "stdout")]
    Stdout,
    #[serde(alias = "stderr")]
    Stderr,
    #[serde(alias = "file")]
    File(String),
    #[serde(alias = "rolling_file")]
    RollingFile(String),
}

#[derive(Deserialize, Clone, Debug)]
pub struct LogConfigPartial {
    root: Option<RootConfig>,
    appenders: Option<Vec<AppenderConfig>>,
    loggers: Option<Vec<LoggerConfig>>,
}

fn default_pattern() -> String {
    String::from(DEFAULT_PATTERN)
}
