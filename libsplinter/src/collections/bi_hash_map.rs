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

use std::borrow::Borrow;
use std::collections::hash_map::Keys;
use std::collections::HashMap;
use std::hash::Hash;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct BiHashMap<K: Hash + Eq, V: Hash + Eq> {
    kv_hash_map: HashMap<K, V>,
    vk_hash_map: HashMap<V, K>,
}

impl<K: Hash + Eq, V: Hash + Eq> BiHashMap<K, V>
where
    K: std::clone::Clone,
    V: std::clone::Clone,
{
    pub fn new() -> Self {
        BiHashMap {
            kv_hash_map: HashMap::new(),
            vk_hash_map: HashMap::new(),
        }
    }

    pub fn keys(&self) -> Keys<K, V> {
        self.kv_hash_map.keys()
    }

    pub fn get_by_key<Q: ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.kv_hash_map.get(key)
    }

    pub fn get_by_value<VQ: ?Sized>(&self, value: &VQ) -> Option<&K>
    where
        V: Borrow<VQ>,
        VQ: Hash + Eq,
    {
        self.vk_hash_map.get(value)
    }

    // return any overridden values, always in (key, value) format
    pub fn insert(&mut self, key: K, value: V) -> (Option<K>, Option<V>) {
        let old_value = self.kv_hash_map.insert(key.clone(), value.clone());
        let old_key = self.vk_hash_map.insert(value, key);
        (old_key, old_value)
    }

    // If the key is in the map, the removed key and value is returned otherwise None
    pub fn remove_by_key<Q: ?Sized>(&mut self, key: &Q) -> Option<(K, V)>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        let value = self.kv_hash_map.remove(key);
        if let Some(value) = value {
            let key = self.vk_hash_map.remove(&value);
            if let Some(key) = key {
                return Some((key, value));
            }
        }
        None
    }

    #[cfg(test)]
    // If the value is in the map, the removed key and value is returned otherwise None
    pub fn remove_by_value<VQ: ?Sized>(&mut self, value: &VQ) -> Option<(K, V)>
    where
        V: Borrow<VQ>,
        VQ: Hash + Eq,
    {
        let key = self.vk_hash_map.remove(value);
        if let Some(key) = key {
            let value = self.kv_hash_map.remove(&key);
            if let Some(value) = value {
                return Some((key, value));
            }
        }
        None
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_insert() {
        let mut map: BiHashMap<String, usize> = BiHashMap::new();
        assert_eq!((None, None), map.insert("ONE".to_string(), 1));
        assert_eq!(
            (Some("ONE".to_string()), Some(1)),
            map.insert("ONE".to_string(), 1)
        );
        assert_eq!(
            (Some("ONE".to_string()), None),
            map.insert("TWO".to_string(), 1)
        );
        assert_eq!((None, Some(1)), map.insert("ONE".to_string(), 3));
    }

    #[test]
    fn test_keys() {
        let mut map: BiHashMap<String, usize> = BiHashMap::new();
        map.insert("ONE".to_string(), 1);
        map.insert("TWO".to_string(), 2);
        map.insert("THREE".to_string(), 3);

        let mut keys: Vec<String> = map.keys().map(|key| key.to_string()).collect();
        keys.sort();
        assert_eq!(
            keys,
            ["ONE".to_string(), "THREE".to_string(), "TWO".to_string()]
        );
    }

    #[test]
    fn test_get() {
        let mut map: BiHashMap<String, usize> = BiHashMap::new();
        map.insert("ONE".to_string(), 1);
        map.insert("TWO".to_string(), 2);
        map.insert("THREE".to_string(), 3);

        assert_eq!(map.get_by_key("ONE"), Some(&1));
        assert_eq!(map.get_by_key("TWO"), Some(&2));
        assert_eq!(map.get_by_key("THREE"), Some(&3));
        assert_eq!(map.get_by_key("FOUR"), None);

        assert_eq!(map.get_by_value(&1), Some(&"ONE".to_string()));
        assert_eq!(map.get_by_value(&2), Some(&"TWO".to_string()));
        assert_eq!(map.get_by_value(&3), Some(&"THREE".to_string()));
        assert_eq!(map.get_by_value(&4), None);
    }

    #[test]
    fn test_removes() {
        let mut map: BiHashMap<String, usize> = BiHashMap::new();
        map.insert("ONE".to_string(), 1);
        map.insert("TWO".to_string(), 2);
        map.insert("THREE".to_string(), 3);

        let removed = map.remove_by_key("ONE");
        assert_eq!(removed, Some(("ONE".to_string(), 1)));

        let removed = map.remove_by_key("ONE");
        assert_eq!(removed, None);

        let removed = map.remove_by_value(&2);
        assert_eq!(removed, Some(("TWO".to_string(), 2)));

        let removed = map.remove_by_value(&2);
        assert_eq!(removed, None);
    }
}
