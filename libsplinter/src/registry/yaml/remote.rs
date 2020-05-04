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

//! A remote, read-only registry.
//!
//! This module contains the [`RemoteYamlRegistry`], which provides an implementation of the
//! [`RegistryReader`] trait.
//!
//! [`RemoteYamlRegistry`]: struct.RemoteYamlRegistry.html
//! [`RegistryReader`]: ../../trait.RegistryReader.html

use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant};

use openssl::hash::{hash, MessageDigest};

use crate::hex::to_hex;
use crate::registry::{
    validate_nodes, MetadataPredicate, Node, NodeIter, RegistryError, RegistryReader,
};

use super::LocalYamlRegistry;

/// A remote, read-only registry.
///
/// The `RemoteYamlRegistry` provides access to a remote registry YAML file over HTTP(S). The remote
/// registry file must be a YAML sequence of nodes, where each node is valid (see [`Node`] for
/// validity criteria). Read operations are provided by the [`RegistryReader`] implementation.
///
/// The remote YAML file is cached locally by saving it to the filesystem. This ensures that the
/// registry will remain available even if the remote file becomes unreachable. The on-disk
/// location of the local cache is determined by the `cache_dir` argument of the registry's
/// [`constructor`].
///
/// On initialization, the `RemoteYamlRegistry` will attempt to immediately fetch and cache the
/// remote file. If this fails, the registry will log an error message and attempt to fetch/cache
/// the remote file every time a read query is made on the registry (through one of the
/// [`RegistryReader`] methods) until the file is successfully cached. Until the registry has a
/// local cache, it will behave as an empty registry. Once the remote file has been successfully
/// cached for the first time, the registry will always provide data from the cache.
///
/// If an `automatic_refresh_period` is provided to the registry's [`constructor`], the registry
/// will attempt to automatically refresh the cache in the background after the given time since
/// the last automatic refresh attempt has elapsed.
///
/// If a `forced_refresh_period` is provided to the registry's [`constructor`], the registry will
/// attempt to cache the remote file on each read after the given time since the last successful
/// cache attempt has elapsed.
///
/// If a forced or automatic cache refresh fails for any reason, an error message will be logged
/// and the previously cached registry values will continue to be used. The next time the registry
/// is read, it will try again to refresh the cache.
///
/// [`Node`]: struct.Node.html
/// [`RegistryReader`]: trait.RegistryReader.html
/// [`constructor`]: struct.RemoteYamlRegistry.html#method.new
pub struct RemoteYamlRegistry {
    internal: Arc<Mutex<Internal>>,
    shutdown_handle: ShutdownHandle,
}

impl RemoteYamlRegistry {
    /// Construct a new `RemoteYamlRegistry`.
    ///
    /// # Arguments
    ///
    /// * `url` - URL of the registry's backing YAML file.
    /// * `cache_dir` - Directory that the local registry cache will be stored in.
    /// * `automatic_refresh_period` - Amount of time between attempts to automatically fetch and
    ///   cache the remote YAML file in the background. If `None`, background refreshes will be
    ///   disabled.
    /// * `forced_refresh_period` - Amount of time since the last successful cache refresh before
    ///   attempting to refresh on every read operation. If `None`, forced refreshes will be
    ///   disabled.
    pub fn new(
        url: &str,
        cache_dir: &str,
        automatic_refresh_period: Option<Duration>,
        forced_refresh_period: Option<Duration>,
    ) -> Result<Self, RegistryError> {
        let internal = Arc::new(Mutex::new(Internal::new(
            url,
            cache_dir,
            forced_refresh_period,
        )?));

        let running = automatic_refresh_period
            .map::<Result<_, RegistryError>, _>(|refresh_period| {
                let running = Arc::new(AtomicBool::new(true));

                let thread_internal = internal.clone();
                let thread_url = url.to_string();
                let thread_running = running.clone();
                thread::Builder::new()
                    .name(format!("Remote Registry Automatic Refresh: {}", url))
                    .spawn(move || {
                        automatic_refresh_loop(
                            refresh_period,
                            thread_internal,
                            &thread_url,
                            thread_running,
                        )
                    })
                    .map_err(|err| {
                        RegistryError::general_error_with_source(
                            &format!(
                                "Failed to spawn automatic refresh thread for remote registry '{}'",
                                url
                            ),
                            Box::new(err),
                        )
                    })?;
                Ok(running)
            })
            .transpose()?;
        let shutdown_handle = ShutdownHandle { running };

        Ok(Self {
            internal,
            shutdown_handle,
        })
    }

    /// Get a copy of the registry's `ShutdownHandle`.
    pub fn shutdown_handle(&self) -> ShutdownHandle {
        self.shutdown_handle.clone()
    }

    /// Acquire the lock for the internal cache and get the nodes from it.
    fn get_nodes(&self) -> Result<Vec<Node>, RegistryError> {
        self.internal
            .lock()
            .map_err(|_| RegistryError::general_error("Internal lock poisoned"))?
            .get_nodes()
    }
}

impl RegistryReader for RemoteYamlRegistry {
    fn fetch_node(&self, identity: &str) -> Result<Option<Node>, RegistryError> {
        Ok(self
            .get_nodes()?
            .into_iter()
            .find(|node| node.identity == identity))
    }

    fn list_nodes<'a, 'b: 'a>(
        &'b self,
        predicates: &'a [MetadataPredicate],
    ) -> Result<NodeIter<'a>, RegistryError> {
        let mut nodes = self.get_nodes()?;
        nodes.retain(|node| predicates.iter().all(|predicate| predicate.apply(node)));
        Ok(Box::new(nodes.into_iter()))
    }

    fn count_nodes(&self, predicates: &[MetadataPredicate]) -> Result<u32, RegistryError> {
        Ok(self
            .get_nodes()?
            .iter()
            .filter(move |node| predicates.iter().all(|predicate| predicate.apply(node)))
            .count() as u32)
    }
}

/// Holds the internal state of the remote registry.
struct Internal {
    url: String,
    cache: LocalYamlRegistry,
    last_refresh_successful: bool,
    forced_refresh_period: Option<Duration>,
    next_forced_refresh: Option<Instant>,
}

impl Internal {
    /// Initialize the internal cache and attempt to populate it immediately.
    fn new(
        url: &str,
        cache_dir: &str,
        forced_refresh_period: Option<Duration>,
    ) -> Result<Self, RegistryError> {
        let url = url.to_string();

        let cache = LocalYamlRegistry::new(&compute_cache_filename(&url, cache_dir)?)?;

        let mut internal = Self {
            url,
            cache,
            last_refresh_successful: false,
            forced_refresh_period,
            next_forced_refresh: None,
        };

        // If initial fetch/cache fails, it will be re-attempted on the next registry read, so just
        // log a message
        if let Err(err) = internal.refresh_cache() {
            warn!(
                "Couldn't initialize cache on startup of remote registry '{}': {}",
                internal.url, err
            );
        }

        Ok(internal)
    }

    /// Attempt to refresh the internal cache and update state accordingly.
    fn refresh_cache(&mut self) -> Result<(), RegistryError> {
        fetch_nodes_from_remote(&self.url)
            .and_then(|nodes| self.cache.write_nodes(nodes))
            .map_err(|err| {
                self.last_refresh_successful = false;
                err
            })
            .and_then(|_| {
                self.last_refresh_successful = true;
                // If a forced refresh period was configured, set the next time a forced refresh
                // will be required
                self.next_forced_refresh = self
                    .forced_refresh_period
                    .map(|duration| {
                        Instant::now().checked_add(duration).ok_or_else(|| {
                            RegistryError::general_error(
                                "Forced refresh time could not be determined; \
                                 forced_refresh_period may be too large",
                            )
                        })
                    })
                    .transpose()?;
                Ok(())
            })
    }

    /// Attempt to refresh the internal cache if necessary and return the cache's contents.
    fn get_nodes(&mut self) -> Result<Vec<Node>, RegistryError> {
        // If the last attempt to refresh the cache wasn't successful, try again
        if !self.last_refresh_successful {
            match self.refresh_cache() {
                Ok(_) => debug!("Successfully refreshed remote registy '{}'", self.url),
                // Last attempt also failed, so just log with DEBUG to keep the WARN logs clean
                Err(err) => debug!("Failed to refresh remote registry '{}': {}", self.url, err),
            }
        }
        // If the forced refresh period has elapsed, attempt to refresh the cache
        else if self
            .next_forced_refresh
            .map(|instant| instant < Instant::now())
            .unwrap_or(false)
        {
            match self.refresh_cache() {
                Ok(_) => debug!("Forced refresh of remote registy '{}' successful", self.url),
                // Already checked that the previous attempt was successful (previous branch of the
                // if/else), so log as WARN to indicate that something's changed
                Err(err) => warn!(
                    "Forced refresh of remote registry '{}' failed: {}",
                    self.url, err
                ),
            }
        }

        self.cache.get_nodes()
    }
}

// Derive the filename for the cache from a hash of the URL; this makes the location deterministic,
// which allows the local cache to be used across restarts.
fn compute_cache_filename(url: &str, cache_dir: &str) -> Result<String, RegistryError> {
    let hash = hash(MessageDigest::sha256(), url.as_bytes())
        .map(|digest| to_hex(&*digest))
        .map_err(|err| {
            RegistryError::general_error_with_source(
                "Failed to hash URL for cache file",
                Box::new(err),
            )
        })?;
    let filename = format!("remote_registry_{}.yaml", hash);
    Ok(Path::new(cache_dir)
        .join(filename)
        .to_str()
        .expect("path built from &str cannot be invalid")
        .to_string())
}

/// Fetch, parse, and validate the YAML registry file at the given URL.
fn fetch_nodes_from_remote(url: &str) -> Result<Vec<Node>, RegistryError> {
    let bytes = reqwest::blocking::get(url)
        .and_then(|response| response.error_for_status())
        .map_err(|err| {
            RegistryError::general_error_with_source(
                &format!("Failed to fetch remote registry file from {}", url),
                Box::new(err),
            )
        })?
        .bytes()
        .map_err(|err| {
            RegistryError::general_error_with_source(
                "Failed to get bytes from remote registry file HTTP response",
                Box::new(err),
            )
        })?;
    let nodes: Vec<Node> = serde_yaml::from_slice(&bytes).map_err(|_| {
        RegistryError::general_error(
            "Failed to deserialize remote registry file: Not a valid YAML sequence of nodes",
        )
    })?;

    validate_nodes(&nodes)?;

    Ok(nodes)
}

/// Infinitely loop, attempting to refresh the `internal` cache every `refresh_period`, until no
/// longer `running`.
fn automatic_refresh_loop(
    refresh_period: Duration,
    internal: Arc<Mutex<Internal>>,
    url: &str,
    running: Arc<AtomicBool>,
) {
    loop {
        // Wait the `refresh_period`, checking for shutdown every second
        let refresh_time = Instant::now() + refresh_period;
        while Instant::now() < refresh_time {
            if !running.load(Ordering::SeqCst) {
                return;
            }
            if let Some(time_left) = refresh_time.checked_duration_since(Instant::now()) {
                thread::sleep(std::cmp::min(time_left, Duration::from_secs(1)));
            }
        }

        let mut internal = match internal.lock() {
            Ok(internal) => internal,
            Err(_) => {
                warn!("Internal lock poisoned for remote registry '{}'", url);
                continue;
            }
        };

        let previous_refresh_successful = internal.last_refresh_successful;

        match internal.refresh_cache() {
            Ok(_) => debug!("Automatic refresh of remote registy '{}' successful", url),
            Err(err) => {
                // If the previous attempt was successful, log with WARN because
                // something changed; if the previous attempt also failed, just log
                // with DEBUG to keep the WARN logs clean.
                let err_msg = format!(
                    "Automatic refresh of remote registry '{}' failed: {}",
                    url, err
                );
                if previous_refresh_successful {
                    warn!("{}", err_msg)
                } else {
                    debug!("{}", err_msg)
                }
            }
        }
    }
}

/// Handle for signaling the `RemoteYamlRegistry` to shutdown.
#[derive(Clone)]
pub struct ShutdownHandle {
    running: Option<Arc<AtomicBool>>,
}

impl ShutdownHandle {
    /// Send shutdown signal to `RemoteYamlRegistry`.
    pub fn shutdown(&self) {
        if let Some(running) = &self.running {
            running.store(false, Ordering::SeqCst)
        }
    }
}
