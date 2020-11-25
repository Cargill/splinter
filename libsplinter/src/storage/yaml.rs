// Copyright 2018-2020 Cargill Incorporated
// Copyright 2018 Bitwise IO, Inc.
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

use std::fmt;
use std::fs::File;
use std::ops::{Deref, DerefMut};

use atomicwrites::{AllowOverwrite, AtomicFile};
use serde::de::DeserializeOwned;
use serde::Serialize;

use super::{Storage, StorageReadGuard, StorageWriteGuard};

/// A yaml read guard
pub struct YamlStorageReadGuard<'a, T: Serialize + DeserializeOwned + 'a> {
    storage: &'a YamlStorage<T>,
}

impl<'a, T: Serialize + DeserializeOwned> YamlStorageReadGuard<'a, T> {
    fn new(storage: &'a YamlStorage<T>) -> Self {
        Self { storage }
    }
}

impl<'a, T: Serialize + DeserializeOwned + 'a> Deref for YamlStorageReadGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.storage.data
    }
}

impl<'a, T: 'a + Serialize + DeserializeOwned + fmt::Display> fmt::Display
    for YamlStorageReadGuard<'a, T>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<'a, T: 'a + Serialize + DeserializeOwned> StorageReadGuard<'a, T>
    for YamlStorageReadGuard<'a, T>
{
}

/// A yaml write guard
pub struct YamlStorageWriteGuard<'a, T: Serialize + DeserializeOwned + 'a> {
    storage: &'a mut YamlStorage<T>,
}

impl<'a, T: Serialize + DeserializeOwned> YamlStorageWriteGuard<'a, T> {
    fn new(storage: &'a mut YamlStorage<T>) -> Self {
        Self { storage }
    }
}

impl<'a, T: Serialize + DeserializeOwned> Drop for YamlStorageWriteGuard<'a, T> {
    fn drop(&mut self) {
        self.storage
            .file
            .write(|f| serde_yaml::to_writer(f, &self.storage.data))
            .expect("File write failed while dropping YamlStorageWriteGuard!");
    }
}

impl<'a, T: Serialize + DeserializeOwned + 'a> Deref for YamlStorageWriteGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.storage.data
    }
}

impl<'a, T: Serialize + DeserializeOwned + 'a> DerefMut for YamlStorageWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.storage.data
    }
}

impl<'a, T: 'a + Serialize + DeserializeOwned + fmt::Display> fmt::Display
    for YamlStorageWriteGuard<'a, T>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<'a, T: 'a + Serialize + DeserializeOwned> StorageWriteGuard<'a, T>
    for YamlStorageWriteGuard<'a, T>
{
}

// A Yaml Storage implementation
///
/// File writes are atomic
pub struct YamlStorage<T: Serialize + DeserializeOwned> {
    data: T,
    file: AtomicFile,
}

impl<T: Serialize + DeserializeOwned> YamlStorage<T> {
    pub fn new<P: Into<String>, F: Fn() -> T>(path: P, default: F) -> Result<Self, String> {
        let path = path.into();

        let file = AtomicFile::new(path, AllowOverwrite);

        // Read the file first, to see if there's any existing data
        let data = match File::open(file.path()) {
            Ok(f) => {
                serde_yaml::from_reader(f).map_err(|err| format!("Couldn't read file: {}", err))?
            }
            Err(_) => {
                let data = default();

                file.write(|f| serde_yaml::to_writer(f, &data))
                    .map_err(|err| format!("File write failed: {}", err))?;

                data
            }
        };

        // Then open the file again and truncate, preparing it to be written to
        Ok(Self { data, file })
    }
}

impl<T: fmt::Display + Serialize + DeserializeOwned> fmt::Display for YamlStorage<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (*self).data.fmt(f)
    }
}

impl<T: Serialize + DeserializeOwned> Storage for YamlStorage<T> {
    type S = T;

    fn read<'a>(&'a self) -> Box<dyn StorageReadGuard<'a, T, Target = T> + 'a> {
        Box::new(YamlStorageReadGuard::new(self))
    }

    fn write<'a>(&'a mut self) -> Box<dyn StorageWriteGuard<'a, T, Target = T> + 'a> {
        Box::new(YamlStorageWriteGuard::new(self))
    }
}
