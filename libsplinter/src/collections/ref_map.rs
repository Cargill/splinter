// Copyright 2018-2020 Cargill Incorporated
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

//! A data structure for reference counting a set of strings.
//!
//! An Item can be added to `RefMap` with `add_ref`; each call to `add_ref` will increment in
//! internal reference count associated with the `ref_id` given. When `remove_ref` is called,
//! the reference count is decremented. If a reference count reaches zero, then the item is
//! removed.

use std::collections::HashMap;

use super::error::RefMapRemoveError;

/// A map that will keep track of the number of times an id has been added, and only remove the
/// id once the reference count is 0.
pub struct RefMap {
    // id to reference count
    references: HashMap<String, u64>,
}

impl RefMap {
    /// Create a new `RefMap`
    pub fn new() -> Self {
        RefMap {
            references: HashMap::new(),
        }
    }

    /// Increments the reference count for `ref_id`
    ///
    /// If `ref_id` does not already exit, it will be added.
    pub fn add_ref(&mut self, ref_id: String) -> u64 {
        if let Some(ref_count) = self.references.remove(&ref_id) {
            let new_ref_count = ref_count + 1;
            self.references.insert(ref_id, new_ref_count);
            new_ref_count
        } else {
            self.references.insert(ref_id, 1);
            1
        }
    }

    /// Decrements the referece count for `ref_id`
    ///
    /// If the internal reference count reaches zero, then `ref_id` will be removed.
    ///
    /// This method will panic if the id does not exist.
    pub fn remove_ref(&mut self, ref_id: &str) -> Result<Option<String>, RefMapRemoveError> {
        let ref_count = match self.references.remove(ref_id) {
            Some(ref_count) => ref_count,
            None => {
                return Err(RefMapRemoveError(format!(
                    "Trying to remove a reference that does not exist: {}",
                    ref_id
                )))
            }
        };

        if ref_count == 1 {
            Ok(Some(ref_id.into()))
        } else {
            self.references.insert(ref_id.into(), ref_count - 1);
            Ok(None)
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    // Test that the reference count is set to 1 if the id is new. If the same id is added, again
    // the reference count is incremented.
    #[test]
    fn test_add_ref() {
        let mut ref_map = RefMap::new();
        let ref_count = ref_map.add_ref("test_id".to_string());
        assert_eq!(ref_count, 1);

        let ref_count = ref_map.add_ref("test_id".to_string());
        assert_eq!(ref_count, 2);

        let ref_count = ref_map.add_ref("test_id_2".to_string());
        assert_eq!(ref_count, 1);
    }

    // Test that when removing a reference, if the ref count is greater than 1, the ref count is
    // is decremented and None is retured to notify that caller that the reference has not be fully
    // removed.
    //
    // Then test that if the ref count is 1, the reference is removed and the id is retured, to
    // tell the caller the reference has been removed.
    #[test]
    fn test_remove_ref() {
        let mut ref_map = RefMap::new();
        let ref_count = ref_map.add_ref("test_id".to_string());
        assert_eq!(ref_count, 1);

        let ref_count = ref_map.add_ref("test_id".to_string());
        assert_eq!(ref_count, 2);

        let id = ref_map.remove_ref("test_id");
        assert_eq!(id, Ok(None));

        assert_eq!(ref_map.references.get("test_id").cloned(), Some(1 as u64));

        let id = ref_map.remove_ref("test_id");
        assert_eq!(id, Ok(Some("test_id".to_string())));
        assert_eq!(ref_map.references.get("test_id"), None);
    }

    // That that if a remove_ref is removed, when the reference does not exist, an error is
    // returned
    #[test]
    fn test_remove_ref_err() {
        let mut ref_map = RefMap::new();
        if let Ok(_) = ref_map.remove_ref("test_id") {
            panic!("remove_ref should have returned an error");
        }
    }
}
