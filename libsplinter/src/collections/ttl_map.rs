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

//! A map whose entries expire after some period of time has elapsed.

use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

/// A map whose entries expire after some period of time has elapsed
pub struct TtlMap<K: Hash + Eq, V> {
    map: HashMap<K, TimedValue<V>>,
    ttl: Duration,
}

impl<K: Hash + Eq, V> TtlMap<K, V> {
    /// Creates an empty `TtlMap` with the given time-to-live for all entries.
    pub fn new(ttl: Duration) -> Self {
        Self {
            map: Default::default(),
            ttl,
        }
    }

    /// Inserts a key-value pair into the map, returning the old value is the key was already
    /// present.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.purge_expired_entries();
        self.map
            .insert(
                key,
                TimedValue {
                    expiration: Instant::now() + self.ttl,
                    value,
                },
            )
            .map(|timed_value| timed_value.value)
    }

    /// Removes a key from the map, returning its value if it was set and has not expired.
    pub fn remove<Q: ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.purge_expired_entries();
        self.map.remove(key).map(|timed_value| timed_value.value)
    }

    /// Checks all entries and removes any that are expired.
    fn purge_expired_entries(&mut self) {
        let now = Instant::now();
        self.map
            .retain(|_, timed_value| timed_value.expiration > now);
    }
}

struct TimedValue<V> {
    expiration: Instant,
    value: V,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that the `TtlMap::insert` method returns the correct values if an entry already
    /// exists or not.
    #[test]
    fn insert() {
        let mut map = TtlMap::new(Duration::from_secs(60));

        assert!(map
            .insert("key".to_string(), "value1".to_string())
            .is_none());
        assert_eq!(
            &map.insert("key".to_string(), "value2".to_string())
                .expect("Entry not found"),
            "value1",
        );
    }

    /// Verifies that the `TtlMap::remove` method returns the correct values if an entry already
    /// exists or not.
    #[test]
    fn remove() {
        let mut map = TtlMap::new(Duration::from_secs(60));

        map.insert("key".to_string(), "value".to_string());

        assert_eq!("value", &map.remove("key").expect("Entry not found"));
        assert!(map.remove("key").is_none());
    }

    /// Verifies that the `TtlMap::insert` method properly purges entries that have expired.
    ///
    /// 1. Create a `TtlMap` with a TTL of 0
    /// 2. Add an entry
    /// 3. Add an entry with the same key using the `insert` method and verify that `None` is
    ///    returned because the old entry expired.
    #[test]
    fn insert_expiration() {
        let mut map = TtlMap::new(Duration::from_secs(0));

        map.insert("key".to_string(), "value1".to_string());
        assert!(map
            .insert("key".to_string(), "value2".to_string())
            .is_none());
    }

    /// Verifies that the `TtlMap::remove` method properly purges entries that have expired.
    ///
    /// 1. Create a `TtlMap` with a TTL of 0
    /// 2. Add an entry
    /// 3. Attempt to remove the entry using the `remove` method and verify that `None` is
    ///    returned because the entry has already expired.
    #[test]
    fn remove_expiration() {
        let mut map = TtlMap::new(Duration::from_secs(0));

        map.insert("key".to_string(), "value".to_string());
        assert!(map.remove("key").is_none());
    }
}
