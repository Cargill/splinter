use log::Level;
use log4rs::append::console::{ConsoleAppender, Target};
use log4rs::append::file::FileAppender;
use log4rs::append::rolling_file::policy::compound::roll::delete::DeleteRoller;
use log4rs::append::rolling_file::policy::compound::trigger::size::SizeTrigger;
use log4rs::append::rolling_file::policy::compound::CompoundPolicy;
use log4rs::append::rolling_file::policy::Policy;
use log4rs::append::rolling_file::RollingFileAppender;
use log4rs::append::Append;
use log4rs::config::Root;
use log4rs::config::{Appender, Config, Logger};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::encode::Encode;
use serde::Deserialize;
use std::default::Default;

#[derive(Deserialize, Clone, Debug)]
pub struct LogConfig {
    root: RootConfig,
    appenders: Vec<AppenderConfig>,
    loggers: Vec<LoggerConfig>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LoggerConfig {
    name: String,
    appenders: Vec<String>,
    filter: Level,
}

#[derive(Deserialize, Clone, Debug)]
pub struct RootConfig {
    appenders: Vec<String>,
    filter: Level,
}

#[derive(Deserialize, Clone, Debug)]
pub struct AppenderConfig {
    name: String,
    #[serde(default = "default_pattern")]
    encoder: String,
    target: LogTarget,
}

#[derive(Deserialize, Clone, Debug)]
pub enum LogTarget {
    StdOut,
    StdErr,
    File(String),
    RollingFile(String),
}

fn default_pattern() -> String {
    String::from("[{d(%Y-%m-%d %H:%M:%S%.3f)}] T[{T}] {l} [{M}] {m}\n")
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            root: Default::default(),
            appenders: vec![Default::default()],
            loggers: Default::default(),
        }
    }
}

impl Default for RootConfig {
    fn default() -> Self {
        Self {
            appenders: vec!["default".to_string()],
            filter: Level::Info,
        }
    }
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self {
            name: String::from("default"),
            appenders: vec![Default::default()],
            filter: Level::Info,
        }
    }
}

impl Default for AppenderConfig {
    fn default() -> Self {
        Self {
            name: String::from("default"),
            encoder: default_pattern(),
            target: Default::default(),
        }
    }
}

impl Default for LogTarget {
    fn default() -> Self {
        LogTarget::StdOut
    }
}

impl AppenderConfig {
    fn try_into_appender(&self) -> Result<Appender, std::io::Error> {
        use LogTarget::*;
        let encoder: Box<dyn Encode> = Box::new(PatternEncoder::new(&self.encoder));
        let boxed: Box<dyn Append> = match &self.target {
            StdOut => Box::new(
                ConsoleAppender::builder()
                    .encoder(encoder)
                    .target(Target::Stdout)
                    .build(),
            ),
            StdErr => Box::new(
                ConsoleAppender::builder()
                    .encoder(encoder)
                    .target(Target::Stderr)
                    .build(),
            ),
            File(path) => Box::new(FileAppender::builder().encoder(encoder).build(path)?),
            RollingFile(path) => {
                let trigger = Box::new(SizeTrigger::new(50_000));
                let roll = Box::new(DeleteRoller::new());
                let policy: Box<dyn Policy> = Box::new(CompoundPolicy::new(trigger, roll));

                Box::new(
                    RollingFileAppender::builder()
                        .encoder(encoder)
                        .build(path, policy)?,
                )
            }
        };
        Ok(Appender::builder().build(&self.name, boxed))
    }
}

// Not using TryInto/TryFrom here because the conversion is not reversable.
impl LoggerConfig {
    fn to_logger(&self) -> Logger {
        let filter = self.filter.to_level_filter();
        Logger::builder()
            .appenders(self.appenders.clone())
            .build(&self.name, filter)
    }
}

impl RootConfig {
    fn to_root_logger(&self) -> Root {
        let filter = self.filter.to_level_filter();
        Root::builder()
            .appenders(self.appenders.clone())
            .build(filter)
    }
    fn set_level(self, level: Level) -> Self {
        Self {
            filter: level,
            ..self
        }
    }
}

impl LogConfig {
    pub fn try_into_config(&self) -> Result<log4rs::Config, log4rs::config::runtime::ConfigErrors> {
        let root = self.root.to_root_logger();
        Config::builder()
            .appenders(
                self.appenders
                    .iter()
                    .filter_map(|ac| ac.try_into_appender().ok()),
            )
            .loggers(self.loggers.iter().map(|lc| lc.to_logger()))
            .build(root)
    }
    pub fn set_root_level(self, level: Level) -> Self {
        Self {
            root: self.root.set_level(level),
            ..self
        }
    }
}
