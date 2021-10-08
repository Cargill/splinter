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

use crate::error::CliError;
use std::convert::From;
use std::fmt;
use std::num::{ParseFloatError, ParseIntError};
use std::time::Duration;
#[derive(Copy, Clone)]

/// A human readable time interval
///
/// supports whole number counts of TimeUnits
pub struct Rate {
    pub numeric: f64,
    pub unit: TimeUnit,
}

impl Rate {
    /// Converts interval to its millisecond representation
    pub fn to_milli(self) -> f64 {
        use TimeUnit::*;
        let mult = match &self.unit {
            Hour => 60.0 * 60_000.0,
            Minute => 60_000.0,
            Second => 1_000.0,
        };
        self.numeric * mult
    }
}

impl fmt::Display for Rate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use TimeUnit::*;
        let unit_part = match self.unit {
            Hour => "h",
            Minute => "m",
            Second => "s",
        };
        write!(f, "{}/{}", self.numeric, unit_part)
    }
}

impl From<f64> for Rate {
    fn from(val: f64) -> Self {
        let numeric = 1_000.0 / val;
        Self {
            numeric,
            unit: TimeUnit::Second,
        }
    }
}

impl std::str::FromStr for Rate {
    type Err = RateParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lowercase = s.to_lowercase().trim().to_string();
        if let Some(parts) = lowercase.split_once('/') {
            let numeric = parts.0.trim().parse::<f64>()?;
            let unit = parts.1.trim().parse::<TimeUnit>()?;
            if numeric >= 0.0 {
                Ok(Self { numeric, unit })
            } else {
                Err(RateParseError {
                    msg: "rate must be positive".to_string(),
                })
            }
        } else {
            lowercase
                .parse::<f64>()
                .map(Rate::from)
                .map_err(|e| e.into())
        }
    }
}

impl PartialEq for Rate {
    fn eq(&self, other: &Self) -> bool {
        self.to_milli() == other.to_milli()
    }
}

impl PartialOrd for Rate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(Duration::from(self).cmp(&Duration::from(other)))
    }
}

impl From<&Rate> for Duration {
    fn from(interval: &Rate) -> Duration {
        Duration::from_secs_f64(interval.numeric / interval.unit.to_sec() as f64)
    }
}

impl From<Rate> for Duration {
    fn from(interval: Rate) -> Duration {
        Duration::from(&interval)
    }
}

/// Supported time units for Rates
#[derive(Copy, Clone)]
pub enum TimeUnit {
    Hour,
    Minute,
    Second,
}

impl std::str::FromStr for TimeUnit {
    type Err = RateParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use TimeUnit::*;
        match s {
            "h" => Ok(Hour),
            "m" => Ok(Minute),
            "s" => Ok(Second),
            _ => Err(RateParseError {
                msg: "Could not parse time unit".to_string(),
            }),
        }
    }
}

impl TimeUnit {
    fn to_sec(self) -> u64 {
        use TimeUnit::*;
        match self {
            Hour => 60 * 60,
            Minute => 60,
            Second => 1,
        }
    }
}

/// Error type for module specific parse errors
#[derive(Debug)]
pub struct RateParseError {
    msg: String,
}

impl std::error::Error for RateParseError {}

impl From<ParseIntError> for RateParseError {
    fn from(error: ParseIntError) -> Self {
        Self {
            msg: format!("{}", error),
        }
    }
}

impl From<ParseFloatError> for RateParseError {
    fn from(error: ParseFloatError) -> Self {
        Self {
            msg: format!("{}", error),
        }
    }
}

impl std::fmt::Display for RateParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unable to parse time {}", self.msg)
    }
}

impl From<RateParseError> for CliError {
    fn from(error: RateParseError) -> Self {
        CliError::UnparseableArg(format!("{}", error))
    }
}
