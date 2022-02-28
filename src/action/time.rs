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
// limitations under the License

use std::convert::From;
use std::fmt;
use std::num::{ParseFloatError, ParseIntError};
use std::str::FromStr;
use std::time::Duration;

use crate::error::CliError;

#[derive(Copy, Clone)]

/// A human readable time interval
///
/// supports whole number counts of TimeUnits
pub struct Time {
    pub numeric: f64,
    pub unit: TimeUnit,
    pub time_type: TimeType,
}

impl Time {
    /// Converts interval to its millisecond representation
    pub fn to_milli(self) -> f64 {
        use TimeUnit::*;
        let mult = match &self.unit {
            Day => 24.0 * 60.0 * 60_000.0,
            Hour => 60.0 * 60_000.0,
            Minute => 60_000.0,
            Second => 1_000.0,
        };
        mult / self.numeric
    }

    pub fn make_duration_type_time(time_str: &str) -> Result<Self, TimeParseError> {
        let t = Time::from_str(time_str)?;
        match t.time_type {
            TimeType::Duration => Ok(t),
            TimeType::Rate => Err(TimeParseError {
                msg: "could not parse duration due to incorrect formatting".into(),
            }),
        }
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use TimeUnit::*;
        let unit_part = match self.unit {
            Day => "d",
            Hour => "h",
            Minute => "m",
            Second => "s",
        };
        match self.time_type {
            TimeType::Rate => {
                write!(f, "{}/{}", self.numeric, unit_part)
            }
            TimeType::Duration => {
                write!(f, "{}{}", self.numeric, unit_part)
            }
        }
    }
}

impl From<f64> for Time {
    fn from(val: f64) -> Self {
        let numeric = 1_000.0 / val;
        Self {
            numeric,
            unit: TimeUnit::Second,
            time_type: TimeType::Rate,
        }
    }
}

impl std::str::FromStr for Time {
    type Err = TimeParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.parse::<f64>().is_err() && !s.contains('/') {
            let (duration_string, unit) = if s.contains('s') {
                (s.replacen('s', "", 1), TimeUnit::Second)
            } else if s.contains('m') {
                (s.replacen('m', "", 1), TimeUnit::Minute)
            } else if s.contains('h') {
                (s.replacen('h', "", 1), TimeUnit::Hour)
            } else if s.contains('d') {
                (s.replacen('d', "", 1), TimeUnit::Day)
            } else {
                return Err(TimeParseError {
                    msg: "could not parse duration due to incorrect formatting".into(),
                });
            };
            let numeric = duration_string.parse::<f64>().map_err(|_| TimeParseError {
                msg: "failed to get numeric value of duration".into(),
            })?;
            Ok(Self {
                numeric,
                unit,
                time_type: TimeType::Duration,
            })
        } else {
            let lowercase = s.to_lowercase().trim().to_string();
            if let Some(parts) = lowercase.split_once('/') {
                let numeric = parts.0.trim().parse::<f64>()?;
                let unit = parts.1.trim().parse::<TimeUnit>()?;
                if numeric >= 0.0 {
                    Ok(Self {
                        numeric,
                        unit,
                        time_type: TimeType::Rate,
                    })
                } else {
                    Err(TimeParseError {
                        msg: "rate must be positive".to_string(),
                    })
                }
            } else {
                lowercase
                    .parse::<f64>()
                    .map(Time::from)
                    .map_err(|e| e.into())
            }
        }
    }
}

impl PartialEq for Time {
    fn eq(&self, other: &Self) -> bool {
        self.to_milli() == other.to_milli()
    }
}

impl PartialOrd for Time {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(Duration::from(self).cmp(&Duration::from(other)))
    }
}

impl From<&Time> for Duration {
    fn from(interval: &Time) -> Duration {
        match interval.time_type {
            TimeType::Rate => {
                Duration::from_secs_f64(interval.unit.to_sec() as f64 / interval.numeric)
            }
            TimeType::Duration => {
                Duration::from_secs_f64(interval.numeric * interval.unit.to_sec() as f64)
            }
        }
    }
}

impl From<Time> for Duration {
    fn from(interval: Time) -> Duration {
        Duration::from(&interval)
    }
}

/// Supported time units for Times
#[derive(Copy, Clone)]
pub enum TimeUnit {
    Day,
    Hour,
    Minute,
    Second,
}

/// Supported time types for Time
#[derive(Copy, Clone)]
pub enum TimeType {
    Rate,
    Duration,
}

impl std::str::FromStr for TimeUnit {
    type Err = TimeParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use TimeUnit::*;
        match s {
            "d" => Ok(Day),
            "h" => Ok(Hour),
            "m" => Ok(Minute),
            "s" => Ok(Second),
            _ => Err(TimeParseError {
                msg: "Could not parse time unit".to_string(),
            }),
        }
    }
}

impl TimeUnit {
    fn to_sec(self) -> u64 {
        use TimeUnit::*;
        match self {
            Day => 60 * 60 * 24,
            Hour => 60 * 60,
            Minute => 60,
            Second => 1,
        }
    }
}

/// Error type for module specific parse errors
#[derive(Debug)]
pub struct TimeParseError {
    msg: String,
}

impl std::error::Error for TimeParseError {}

impl From<ParseIntError> for TimeParseError {
    fn from(error: ParseIntError) -> Self {
        Self {
            msg: format!("{}", error),
        }
    }
}

impl From<ParseFloatError> for TimeParseError {
    fn from(error: ParseFloatError) -> Self {
        Self {
            msg: format!("{}", error),
        }
    }
}

impl std::fmt::Display for TimeParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unable to parse time {}", self.msg)
    }
}

impl From<TimeParseError> for CliError {
    fn from(error: TimeParseError) -> Self {
        CliError::UnparseableArg(format!("{}", error))
    }
}
