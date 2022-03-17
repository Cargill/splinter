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

use std::convert::TryFrom;
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};

use crate::error::InvalidArgumentError;

/// Defines the validation rules for a given service type.
///
/// This is a macro, due to the due to the inability to use different error types (panic vs
/// InvalidArgumentError) in the const function vs the heap function.
macro_rules! validate_service_type {
    ($service_type:ident, $err_macro:ident) => {
        if $service_type.is_empty() {
            $err_macro!("A ServiceType cannot be empty");
        }
        if $service_type.len() > 64 {
            $err_macro!("A ServiceType must be between 1 and 64 characters");
        }

        let bytes = $service_type.as_bytes();
        let mut i = 0;
        let mut version_index = bytes.len();
        while i < bytes.len() {
            let b = bytes[i];

            if b == b':' && version_index == bytes.len() {
                version_index = i;
            } else if !valid_service_name_char(b) {
                $err_macro!("A ServiceType must be alphanumeric");
            }
            i += 1;
        }
    };
}

/// A convenience for the InvalidArgumentError so as to be swappable with the panic macro
/// (the only argument to the constructors is service_type)
macro_rules! invalid_arg_error {
    ($msg:literal) => {
        return Err(InvalidArgumentError::new("service_type", $msg));
    };
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ServiceType<'a>(ServiceTypeInner<'a>);

impl<'a> ServiceType<'a> {
    /// Construct a static `ServiceType`, which may be used as constant.
    ///
    /// This value panics vs returning an error, as the static string can be validated at compile
    /// time.
    #[track_caller]
    pub const fn new_static(service_type: &'static str) -> ServiceType<'static> {
        validate_service_type!(service_type, panic);
        ServiceType(ServiceTypeInner::Borrowed(service_type))
    }

    /// Constructs a new `ServiceType`.
    ///
    /// # Errors
    ///
    /// Returns a [`InvalidArgumentError`] if
    ///
    /// * The input string is empty
    /// * The input string is too long
    /// * The input string is not alphanumeric, or in the format "type:version"
    pub fn new<S: Into<String>>(service_type: S) -> Result<Self, InvalidArgumentError> {
        Self::new_box_str(service_type.into().into_boxed_str())
    }

    /// An inner constructor
    fn new_box_str(service_type: Box<str>) -> Result<Self, InvalidArgumentError> {
        validate_service_type!(service_type, invalid_arg_error);

        Ok(ServiceType(ServiceTypeInner::Owned(service_type)))
    }

    /// Return the service type name
    pub fn service_type_name(&self) -> &str {
        match self.0.service_type().split(':').next() {
            Some(service_type_name) => service_type_name,
            // This iterator always returns at least one value
            None => unreachable!(),
        }
    }

    /// Return the version, if specified.
    pub fn version(&self) -> Option<&str> {
        // skip the first value
        self.0.service_type().split(':').nth(1)
    }
}

impl<'a> TryFrom<String> for ServiceType<'a> {
    type Error = InvalidArgumentError;

    fn try_from(service_type: String) -> Result<Self, Self::Error> {
        ServiceType::new(service_type)
    }
}

impl<'a> TryFrom<&str> for ServiceType<'a> {
    type Error = InvalidArgumentError;

    fn try_from(service_type: &str) -> Result<Self, Self::Error> {
        ServiceType::new(service_type)
    }
}

impl<'a> TryFrom<Box<str>> for ServiceType<'a> {
    type Error = InvalidArgumentError;

    fn try_from(service_type: Box<str>) -> Result<Self, Self::Error> {
        ServiceType::new_box_str(service_type)
    }
}

const fn valid_service_name_char(b: u8) -> bool {
    ((b'0' <= b) && (b <= b'9' )) // numeric
        || ((b'A' <= b) && (b<= b'Z')) // uppercase
        || ((b'a' <= b) && (b <= b'z')) // lowercase
}

impl<'a> Display for ServiceType<'a> {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match &self.0 {
            ServiceTypeInner::Borrowed(service_type) => f.write_str(service_type),
            ServiceTypeInner::Owned(service_type) => f.write_str(&**service_type),
        }
    }
}

#[derive(Clone)]
enum ServiceTypeInner<'a> {
    Borrowed(&'a str),
    Owned(Box<str>),
}

impl<'a> ServiceTypeInner<'a> {
    fn service_type(&self) -> &str {
        match self {
            ServiceTypeInner::Borrowed(service_type) => service_type,
            ServiceTypeInner::Owned(service_type) => &**service_type,
        }
    }
}

impl<'a> Debug for ServiceTypeInner<'a> {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            ServiceTypeInner::Borrowed(service_type) => write!(f, "{:?}", service_type),
            ServiceTypeInner::Owned(service_type) => write!(f, "{:?}", service_type),
        }
    }
}

impl<'a> PartialEq for ServiceTypeInner<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.service_type() == other.service_type()
    }
}

impl<'a> Eq for ServiceTypeInner<'a> {}

#[cfg(test)]
mod tests {
    use super::*;

    const STATIC_TYPE: ServiceType = ServiceType::new_static("statictype");
    const STATIC_TYPE_WITH_VERSION: ServiceType = ServiceType::new_static("statictype:v2");

    /// This test ensures that use of static const ServiceType values may be used.
    #[test]
    fn test_new_static() {
        assert_eq!(STATIC_TYPE.to_string(), String::from("statictype"));
    }

    /// This test constructs a value from a string.
    #[test]
    fn test_new_string() {
        let service_type = ServiceType::new("stringtype").unwrap();
        assert_eq!(service_type.to_string(), String::from("stringtype"));
    }

    /// This test validates debug output from both methods of constructing a ServiceType.
    #[test]
    fn test_debug_output() {
        assert_eq!(format!("{:?}", STATIC_TYPE), r#"ServiceType("statictype")"#);
        let service_type = ServiceType::new("stringtype").unwrap();
        assert_eq!(
            format!("{:?}", service_type),
            r#"ServiceType("stringtype")"#
        );
    }

    #[test]
    fn test_new_with_version() {
        let service_type = ServiceType::new("service:v2").unwrap();
        assert_eq!(service_type.to_string(), String::from("service:v2"));
        assert_eq!(service_type.service_type_name(), "service");
        assert_eq!(service_type.version(), Some("v2"));
    }

    #[test]
    fn test_static_with_version() {
        assert_eq!(
            STATIC_TYPE_WITH_VERSION.to_string(),
            String::from("statictype:v2")
        );
        assert_eq!(STATIC_TYPE_WITH_VERSION.service_type_name(), "statictype");
        assert_eq!(STATIC_TYPE_WITH_VERSION.version(), Some("v2"));
    }

    #[test]
    #[should_panic(expected = "A ServiceType cannot be empty")]
    fn test_static_zero_length_failure() {
        ServiceType::new_static("");
    }

    #[test]
    fn test_new_zero_length_failure() {
        assert_eq!(
            ServiceType::new("").unwrap_err().to_string(),
            "A ServiceType cannot be empty (service_type)"
        );
    }

    #[test]
    #[should_panic(expected = "A ServiceType must be between 1 and 64 characters")]
    fn test_static_too_long_failure() {
        ServiceType::new_static(
            "0123456789012345678901234567890123456789012345678901234567890123456789",
        );
    }

    #[test]
    fn test_new_too_long_failure() {
        assert_eq!(
            ServiceType::new(
                "0123456789012345678901234567890123456789012345678901234567890123456789",
            )
            .unwrap_err()
            .to_string(),
            "A ServiceType must be between 1 and 64 characters (service_type)"
        );
    }

    #[test]
    #[should_panic(expected = "A ServiceType must be alphanumeric")]
    fn test_static_alphanumeric_only() {
        ServiceType::new_static("#service");
    }
    #[test]
    fn test_new_alphanumeric_only() {
        assert_eq!(
            ServiceType::new("#service").unwrap_err().to_string(),
            "A ServiceType must be alphanumeric (service_type)"
        );
    }

    /// This test validates comparisons between static types and string types.
    #[test]
    fn test_comparisons() {
        let compare_type = ServiceType::new("statictype").unwrap();
        let other_type = ServiceType::new("othertype").unwrap();
        let other_type2 = ServiceType::new("othertype").unwrap();

        assert_eq!(compare_type, STATIC_TYPE);
        assert_eq!(other_type, other_type2);
        assert!(other_type != STATIC_TYPE);
        assert!(other_type != compare_type);
    }
}
