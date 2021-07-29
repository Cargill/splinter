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

use std::str::FromStr;

use serde::de::Visitor;
use serde::Deserialize;

#[derive(Clone, Debug)]
pub struct ByteSize {
    size: f32,
    unit: MemoryUnitSize,
}

impl ByteSize {
    pub fn get_mem_size(&self) -> u64 {
        (self.size * self.unit.byte_count() as f32).trunc() as u64
    }
}

#[derive(Clone, Debug)]
enum MemoryUnitSize {
    Kilobyte,
    Megabyte,
    Gigabyte,
}

impl FromStr for MemoryUnitSize {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "M" => Ok(Self::Megabyte),
            "K" => Ok(Self::Kilobyte),
            "G" => Ok(Self::Gigabyte),
            _ => Err(()),
        }
    }
}

impl MemoryUnitSize {
    fn byte_count(&self) -> u64 {
        match self {
            Self::Kilobyte => 1_000,
            Self::Megabyte => 1_000_000,
            Self::Gigabyte => 1_000_000_000,
        }
    }
}

impl<'de> Deserialize<'de> for ByteSize {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_string(ByteSizeVisitor)
    }
}

struct ByteSizeVisitor;

impl<'de> Visitor<'de> for ByteSizeVisitor {
    type Value = ByteSize;
    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        // serde calls these methods hints and its not always clear which method gets used. Hence
        // the visit_string and visitr_str methods both being defined.
        self.visit_str(&v)
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        //floats support a bunch of different formats, this supports <digit[s]>.<digit[s]>
        let numeric: Result<f32, _> = v
            .chars()
            .take_while(|x| x.is_digit(10) || *x == '.')
            .collect::<String>()
            .parse();
        // Units can be K,M,G for kilo, mega, giga bytes.
        let units: Result<MemoryUnitSize, _> = v
            .chars()
            .skip_while(|x| x.is_digit(10) || *x == '.')
            .take_while(|c| c.is_alphabetic())
            .collect::<String>()
            .parse();
        match (numeric, units) {
            (Ok(size), Ok(unit)) => Ok(ByteSize { size, unit }),
            (Err(e), _) => Err(E::custom(format!("size could not be parsed: {}", e))),
            (_, Err(_)) => Err(E::custom("unit could not be parsed".to_string())),
        }
    }
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "<float><K|M|G>")
    }
}
