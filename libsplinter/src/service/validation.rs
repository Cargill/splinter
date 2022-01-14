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

//! Service Argument validation

use std::collections::HashMap;

use crate::error::InvalidArgumentError;

type Args = HashMap<String, String>;
type ValidationResult = Result<(), InvalidArgumentError>;

/// Validates the arguments for a service
pub trait ServiceArgValidator {
    /// Validate the given arguments.
    ///
    /// # Errors
    ///
    /// Returns an InvalidArgumentError if the implementation determines that the arguments
    /// are invalid.
    fn validate(&self, args: &Args) -> ValidationResult;
}

// Implement the trait on all boxed-dyn ServiceArgValidators
impl ServiceArgValidator for Box<dyn ServiceArgValidator> {
    fn validate(&self, args: &Args) -> ValidationResult {
        (**self).validate(args)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    struct ContainsFoo;

    impl ServiceArgValidator for ContainsFoo {
        fn validate(&self, args: &Args) -> ValidationResult {
            if !args.contains_key("foo") {
                return Err(InvalidArgumentError::new(
                    "foo".into(),
                    r#""foo" is missing"#.into(),
                ));
            }

            Ok(())
        }
    }

    struct BarNotEmpty;

    impl ServiceArgValidator for BarNotEmpty {
        fn validate(&self, args: &Args) -> ValidationResult {
            if args.get("bar").map(|v| v.is_empty()).unwrap_or(true) {
                return Err(InvalidArgumentError::new(
                    "bar".into(),
                    r#""bar" is missing or empty"#.into(),
                ));
            }

            Ok(())
        }
    }

    /// Test that a valid set of arguments passes both validations.
    #[test]
    fn test_valid() {
        let validators: Vec<Box<dyn ServiceArgValidator>> =
            vec![Box::new(ContainsFoo), Box::new(BarNotEmpty)];

        let mut args: Args = HashMap::new();
        args.insert("foo".into(), "one".into());
        args.insert("bar".into(), "yes".into());

        assert!(validators
            .iter()
            .map(|v| v.validate(&args))
            .all(|r| r.is_ok()))
    }

    /// Test that a set of arguments missing the "foo" argument will fail with an error.
    #[test]
    fn test_fail_contains_validation() {
        let validators: Vec<Box<dyn ServiceArgValidator>> =
            vec![Box::new(ContainsFoo), Box::new(BarNotEmpty)];

        let mut args: Args = HashMap::new();
        args.insert("bar".into(), "yes".into());

        let _expected_result: Option<Result<(), InvalidArgumentError>> = Some(Err(
            InvalidArgumentError::new("foo".into(), "argument is missing".into()),
        ));

        assert!(matches!(
            validators
                .iter()
                .map(|v| v.validate(&args))
                .find(|r| r.is_err()),
            _expected_result,
        ));
    }

    /// Test that a set of arguments with an invalid "bar" argument will fail with an error.
    #[test]
    fn test_fail_not_empty_validation() {
        let validators: Vec<Box<dyn ServiceArgValidator>> =
            vec![Box::new(ContainsFoo), Box::new(BarNotEmpty)];

        let mut args: Args = HashMap::new();
        args.insert("foo".into(), "one".into());
        args.insert("bar".into(), "".into());

        let _expected_result: Option<Result<(), InvalidArgumentError>> = Some(Err(
            InvalidArgumentError::new("bar".into(), "missing or empty".into()),
        ));

        assert!(matches!(
            validators
                .iter()
                .map(|v| v.validate(&args))
                .find(|r| r.is_err()),
            _expected_result
        ));
    }
}
