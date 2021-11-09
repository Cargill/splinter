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

use serde::de::Visitor;
use serde::Deserialize;

#[derive(Clone, Debug)]
pub struct ByteSize {
    size: u64,
}

impl From<ByteSize> for u64 {
    fn from(bytes: ByteSize) -> Self {
        bytes.size
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
        let multiple = v
            .chars()
            .skip_while(|x| x.is_digit(10) || *x == '.')
            .take_while(|c| c.is_alphabetic())
            .collect::<String>();
        let multiple = match multiple.as_str() {
            "M" => Ok(1_000_000),
            "K" => Ok(1_000),
            "G" => Ok(1_000_000_000),
            _ => Err(E::custom("unit could not be parsed".to_string())),
        };
        match (numeric, multiple) {
            (Ok(float), Ok(mult)) => Ok(ByteSize {
                size: (float * mult as f32).trunc() as u64,
            }),
            (Err(e), _) => Err(E::custom(format!("size could not be parsed: {}", e))),
            (_, Err(e)) => Err(e),
        }
    }
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "<float><K|M|G>")
    }
}
