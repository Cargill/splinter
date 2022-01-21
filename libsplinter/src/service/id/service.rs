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

//! Implementation of service identifier, ServiceId.

use std::convert::TryFrom;
use std::fmt;

use rand::{distributions::Alphanumeric, Rng};

use crate::error::InvalidArgumentError;

/// A service identifier.
///
/// A service id consists of a string, with one of the following formats:
///
/// - 4 character alphanumeric string (non-management circuits)
/// - a public key hex string (management circuit only)
/// - a node identifier (management circuit only)
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ServiceId {
    inner: Box<str>,
}

impl ServiceId {
    /// Create a `ServiceId` from a string.
    ///
    /// # Arguments
    ///
    /// * `service_id` - A alphanumeric string of representing a service identifier.
    ///
    /// # Errors
    ///
    /// [`InvalidArgumentError`] can occur when the string contains invalid characters (only
    /// alphanumeric characters are allowed).
    pub fn new<T: Into<String>>(service_id: T) -> Result<Self, InvalidArgumentError> {
        ServiceId::new_box_str(service_id.into().into_boxed_str())
    }

    /// Creates a new `ServiceId` with a random value.
    pub fn new_random() -> Self {
        let mut rng = rand::thread_rng();

        let service_id_str: String = std::iter::repeat(())
            .map(|()| rng.sample(Alphanumeric))
            .map(char::from)
            .take(4)
            .collect();

        ServiceId {
            inner: service_id_str.into(),
        }
    }

    // Private as external API for this is to use try_from.
    fn new_box_str(service_id: Box<str>) -> Result<Self, InvalidArgumentError> {
        if !service_id.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(InvalidArgumentError::new(
                "service_id".to_string(),
                "invalid characters, expected ASCII alphanumeric".to_string(),
            ));
        }

        Ok(ServiceId { inner: service_id })
    }

    /// Returns a `&str` representing the value of ServiceId.
    pub fn as_str(&self) -> &str {
        &self.inner
    }

    /// Returns a `Box<str>`, consuming the ServiceId.
    pub fn deconstruct(self) -> Box<str> {
        self.inner
    }
}

impl TryFrom<String> for ServiceId {
    type Error = InvalidArgumentError;

    fn try_from(service_id: String) -> Result<Self, Self::Error> {
        ServiceId::new(service_id)
    }
}

impl TryFrom<Box<str>> for ServiceId {
    type Error = InvalidArgumentError;

    fn try_from(service_id: Box<str>) -> Result<Self, Self::Error> {
        ServiceId::new_box_str(service_id)
    }
}

impl TryFrom<&str> for ServiceId {
    type Error = InvalidArgumentError;

    fn try_from(service_id: &str) -> Result<Self, Self::Error> {
        ServiceId::new(service_id)
    }
}

impl fmt::Display for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::convert::{TryFrom, TryInto};

    use crate::error::InvalidArgumentError;

    use super::ServiceId;

    /// Tests successfully creating a ServiceId from a well-formed `&str` using ServiceId::new().
    #[test]
    fn test_service_id_well_formed_new_str() {
        let service_id = ServiceId::new("abcd").expect("creating ServiceId from \"abcd\"");
        assert_eq!(service_id.as_str(), "abcd")
    }

    /// Tests successfully creating a ServiceId from a well-formed `Box<str>` using ServiceId::new().
    #[test]
    fn test_service_id_well_formed_new_box_str() {
        let s: Box<str> = "abc0".into();
        let service_id = ServiceId::new(s).expect("creating ServiceId from \"abc0\"");
        assert_eq!(service_id.as_str(), "abc0");
    }

    /// Tests successfully creating a ServiceId from a well-formed `String` using ServiceId::new().
    #[test]
    fn test_service_id_well_formed_new_string() {
        let service_id =
            ServiceId::new(String::from("abcd")).expect("creating ServiceId from \"abcd\"");
        assert_eq!(service_id.as_str(), "abcd")
    }

    /// Tests successfully creating a ServiceId from a well-formed `String` using ServiceId::try_from().
    #[test]
    fn test_service_id_well_formed_try_from_string() {
        let service_id =
            ServiceId::try_from(String::from("abcd")).expect("creating ServiceId from \"abcd\"");
        assert_eq!(service_id.as_str(), "abcd")
    }

    /// Tests successfully creating a ServiceId from a well-formed `String` using String::try_into().
    #[test]
    fn test_service_id_well_formed_try_into_string() {
        let service_id: ServiceId = String::from("abcd")
            .try_into()
            .expect("creating ServiceId from \"abcd\"");
        assert_eq!(service_id.as_str(), "abcd")
    }

    /// Tests successfully creating a ServiceId from a well-fromed `Box<str>` using Box<str>::try_into().
    #[test]
    fn test_service_id_well_formed_from_box_str() {
        let s: Box<str> = "abcd".into();
        let service_id: ServiceId = s.try_into().expect("creating ServiceId from \"abcd\"");
        assert_eq!(service_id.as_str(), "abcd")
    }

    /// Tests successfully creating a ServiceId, deconstructing, and turning it back into
    /// a ServiceId while avoiding the need to allocate a String.
    #[test]
    fn test_service_id_round_trip_without_string() {
        let service_id: ServiceId = ServiceId::new("abcd")
            .expect("creating ServiceId from \"abcd\"")
            .deconstruct()
            .try_into()
            .expect("creating ServiceId from \"abcd\"");
        assert_eq!(service_id.as_str(), "abcd");
    }

    /// Tests for error creating a ServiceId with invalid characters using ServiceId::new().
    #[test]
    fn test_service_id_invalid_characters_new() {
        assert_eq!(
            &ServiceId::new("abcd!").unwrap_err().to_string(),
            "invalid characters, expected ASCII alphanumeric (service_id)",
        );
    }

    /// Tests for error creating a ServiceId with invalid characters using String::try_into().
    #[test]
    fn test_service_id_invalid_characters_from_string() {
        let result: Result<ServiceId, InvalidArgumentError> = String::from("a-cde").try_into();
        assert_eq!(
            &result.unwrap_err().to_string(),
            "invalid characters, expected ASCII alphanumeric (service_id)",
        );
    }

    /// Tests for error creating a ServiceId with invalid characters using Box<str>::try_into().
    #[test]
    fn test_service_id_invalid_characters_from_box_str() {
        let s: Box<str> = "ab(de".into();
        let result: Result<ServiceId, InvalidArgumentError> = s.try_into();
        assert_eq!(
            &result.unwrap_err().to_string(),
            "invalid characters, expected ASCII alphanumeric (service_id)",
        );
    }

    /// Test successfully creation ServiceIds with ServiceId::new_random().
    ///
    /// Calls new_random() many times and verifies that the results are reasonably random (that,
    /// for example, we aren't getting the same ServiceId every time).
    #[test]
    fn test_service_id_new_random() {
        const ITERATIONS: usize = 100_000;

        let mut set = HashSet::new();

        for _ in 0..ITERATIONS {
            let service_id = ServiceId::new_random();
            assert_eq!(service_id.as_str().len(), 4);
            assert!(service_id
                .as_str()
                .chars()
                .all(|c| c.is_ascii_alphanumeric()));

            set.insert(service_id);
        }

        // Check that the set is of a reasonable size. Because there may be overlap due to the
        // random nature of new_random(), we check the length of the set is greater than 90% of the
        // number of iterations.
        assert!(set.len() > (ITERATIONS as f32 * 0.9) as usize);
    }

    /// Test that ServiceId is displayed as its alphanumberic string of characters.
    #[test]
    fn test_service_id_display_value() {
        let service_id: ServiceId = String::from("abcde")
            .try_into()
            .expect("creating a ServiceId from \"abcde\"");
        assert_eq!(&format!("{}", service_id), "abcde");
    }
}
