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
    ///   disabled. The automatic refresh occurs with a tolerance of +/- 1 second.
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
                Ok(_) => debug!("Successfully refreshed remote registry '{}'", self.url),
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
                Ok(_) => debug!(
                    "Forced refresh of remote registry '{}' successful",
                    self.url
                ),
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
            Ok(_) => debug!("Automatic refresh of remote registry '{}' successful", url),
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

#[cfg(all(test, feature = "rest-api", feature = "rest-api-actix"))]
mod tests {
    use super::*;

    use std::fs::File;

    use actix_web::HttpResponse;
    use futures::future::IntoFuture;
    use tempdir::TempDir;

    use crate::rest_api::{
        Method, Resource, RestApiBuilder, RestApiServerError, RestApiShutdownHandle,
    };

    /// Verifies that a remote file that contains two nodes with the same identity is rejected (not
    /// loaded).
    #[test]
    fn duplicate_identity() {
        let mut registry = mock_registry();
        registry[0].identity = "identity".into();
        registry[1].identity = "identity".into();
        let test_config = TestConfig::setup("duplicate_identity", Some(registry));

        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        // Verify that the registry is still empty
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    /// Verifies that a remote file that contains two nodes with the same endpoint is rejected (not
    /// loaded).
    #[test]
    fn duplicate_endpoint() {
        let mut registry = mock_registry();
        registry[0].endpoints = vec!["endpoint".into()];
        registry[1].endpoints = vec!["endpoint".into()];
        let test_config = TestConfig::setup("duplicate_endpoint", Some(registry));

        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        // Verify that the registry is still empty
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    /// Verifies that a remote file that contains a node with an empty string as its identity is
    /// rejected (not loaded).
    #[test]
    fn empty_identity() {
        let mut registry = mock_registry();
        registry[0].identity = "".into();
        let test_config = TestConfig::setup("empty_identity", Some(registry));

        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        // Verify that the registry is still empty
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    /// Verifies that a remote file that contains a node with an empty string in its endpoints is
    /// rejected (not loaded).
    #[test]
    fn empty_endpoint() {
        let mut registry = mock_registry();
        registry[0].endpoints = vec!["".into()];
        let test_config = TestConfig::setup("empty_endpoint", Some(registry));

        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        // Verify that the registry is still empty
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    /// Verifies that a remote file that contains a node with an empty string as its display name is
    /// rejected (not loaded).
    #[test]
    fn empty_display_name() {
        let mut registry = mock_registry();
        registry[0].display_name = "".into();
        let test_config = TestConfig::setup("empty_display_name", Some(registry));

        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        // Verify that the registry is still empty
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    /// Verifies that a remote file that contains a node with an empty string in its keys is
    /// rejected (not loaded).
    #[test]
    fn empty_key() {
        let mut registry = mock_registry();
        registry[0].keys = vec!["".into()];
        let test_config = TestConfig::setup("empty_key", Some(registry));

        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        // Verify that the registry is still empty
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    /// Verifies that a remote file that contains a node with no endpoints is rejected (not loaded).
    #[test]
    fn missing_endpoints() {
        let mut registry = mock_registry();
        registry[0].endpoints = vec![];
        let test_config = TestConfig::setup("missing_endpoints", Some(registry));

        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        // Verify that the registry is still empty
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    /// Verifies that a remote file that contains a node with no keys is rejected (not loaded).
    #[test]
    fn missing_keys() {
        let mut registry = mock_registry();
        registry[0].keys = vec![];
        let test_config = TestConfig::setup("missing_keys", Some(registry));

        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        // Verify that the registry is still empty
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    /// Verifies that `fetch_node` with an existing identity returns the correct node.
    #[test]
    fn fetch_node_ok() {
        let test_config = TestConfig::setup("fetch_node_ok", Some(mock_registry()));

        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        let expected_node = mock_registry().pop().expect("Failed to get expected node");
        let node = remote_registry
            .fetch_node(&expected_node.identity)
            .expect("Failed to fetch node")
            .expect("Node not found");
        assert_eq!(node, expected_node);

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    /// Verifies that `fetch_node` with a non-existent identity returns Ok(None)
    #[test]
    fn fetch_node_not_found() {
        let test_config = TestConfig::setup("fetch_node_not_found", Some(mock_registry()));

        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        assert!(remote_registry
            .fetch_node("NodeNotInRegistry")
            .expect("Failed to fetch node")
            .is_none());

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    ///
    /// Verifies that `has_node` properly determines if a node exists in the registry.
    ///
    #[test]
    fn has_node() {
        let test_config = TestConfig::setup("has_node", Some(mock_registry()));

        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        let expected_node = mock_registry().pop().expect("Failed to get expected node");
        assert!(remote_registry
            .has_node(&expected_node.identity)
            .expect("Failed to check if expected_node exists"));
        assert!(!remote_registry
            .has_node("NodeNotInRegistry")
            .expect("Failed to check for non-existent node"));

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    /// Verifies that `list_nodes` returns all nodes in the remote file.
    #[test]
    fn list_nodes() {
        let test_config = TestConfig::setup("list_nodes", Some(mock_registry()));

        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        let nodes = remote_registry
            .list_nodes(&[])
            .expect("Failed to retrieve nodes")
            .collect::<Vec<_>>();

        assert_eq!(nodes.len(), mock_registry().len());
        for node in mock_registry() {
            assert!(nodes.contains(&node));
        }

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    /// Verifies that `list_nodes` returns an empty list when there are no nodes in the remote file.
    #[test]
    fn list_nodes_empty() {
        let test_config = TestConfig::setup("list_nodes_empty", Some(vec![]));

        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        let nodes = remote_registry
            .list_nodes(&[])
            .expect("Failed to retrieve nodes")
            .collect::<Vec<_>>();

        assert!(nodes.is_empty());

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    /// Verifies that `list_nodes` returns the correct nodes when a metadata filter is provided.
    #[test]
    fn list_nodes_filter_metadata() {
        let test_config = TestConfig::setup("list_nodes_filter_metadata", Some(mock_registry()));

        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        let filter = vec![MetadataPredicate::Eq(
            "company".into(),
            mock_registry()[0]
                .metadata
                .get("company")
                .expect("company metadata not set")
                .into(),
        )];

        let nodes = remote_registry
            .list_nodes(&filter)
            .expect("Failed to retrieve nodes")
            .collect::<Vec<_>>();

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0], mock_registry()[0]);

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    /// Verifies that `list_nodes` returns the correct nodes when multiple metadata filters are
    /// provided.
    #[test]
    fn list_nodes_filter_multiple() {
        let test_config = TestConfig::setup("list_nodes_filter_multiple", Some(mock_registry()));

        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        let filter = vec![
            MetadataPredicate::Eq(
                "company".to_string(),
                mock_registry()[2]
                    .metadata
                    .get("company")
                    .unwrap()
                    .to_string(),
            ),
            MetadataPredicate::Eq(
                "admin".to_string(),
                mock_registry()[2]
                    .metadata
                    .get("admin")
                    .unwrap()
                    .to_string(),
            ),
        ];

        let nodes = remote_registry
            .list_nodes(&filter)
            .expect("Failed to retrieve nodes")
            .collect::<Vec<_>>();

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0], mock_registry()[2]);

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    /// Verifies that `list_nodes` returns an empty list when no nodes fit the filtering criteria.
    #[test]
    fn list_nodes_filter_empty() {
        let test_config = TestConfig::setup("list_nodes_filter_empty", Some(mock_registry()));

        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        let filter = vec![MetadataPredicate::Eq(
            "admin".to_string(),
            "not an admin".to_string(),
        )];

        let nodes = remote_registry
            .list_nodes(&filter)
            .expect("Failed to retrieve nodes")
            .collect::<Vec<_>>();

        assert!(nodes.is_empty());

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    /// Verifies that when the remote file is available at startup, it's fetched and cached
    /// successfully. The internal list of nodes and the backing file should match the remote file.
    #[test]
    fn file_available_at_startup() {
        let test_config = TestConfig::setup("file_available_at_startup", Some(mock_registry()));

        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        verify_internal_cache(&test_config, &remote_registry, mock_registry());

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    /// Verifies that when the remote file is not available at startup, the registry starts up with
    /// an empty cache. When the remote file becomes available, it should be fetched and cached on
    /// the next read.
    #[test]
    fn file_unavailable_at_startup() {
        // Start without a remote file
        let test_config = TestConfig::setup("file_unavailable_at_startup", None);

        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        // Verify that the registry is still empty
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        // Make the remote file available now
        test_config.update_registry(Some(mock_registry()));

        // Verify that the registry's contents were updated
        verify_internal_cache(&test_config, &remote_registry, mock_registry());

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    /// Verifies that when auto refresh is turned off, the auto refresh thread is not running.
    #[test]
    fn auto_refresh_disabled() {
        let test_config = TestConfig::setup("auto_refresh_disabled", Some(mock_registry()));

        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        // The `running` atomic bool is only set if the auto refresh thread was started.
        assert!(remote_registry.shutdown_handle().running.is_none());

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    /// Verifies that when auto refresh is turned on, the auto refresh thread is running and
    /// refreshes the registry in the background
    #[test]
    fn auto_refresh_enabled() {
        let test_config = TestConfig::setup("auto_refresh_enabled", Some(mock_registry()));

        let refresh_period = Duration::from_secs(1);
        let remote_registry = RemoteYamlRegistry::new(
            test_config.url(),
            test_config.path(),
            Some(refresh_period),
            None,
        )
        .expect("Failed to create registry");

        verify_internal_cache(&test_config, &remote_registry, mock_registry());

        // The `running` atomic bool is only set if the auto refresh thread was started.
        assert!(remote_registry.shutdown_handle().running.is_some());

        test_config.update_registry(Some(vec![]));

        // Wait twice as long as the auto refresh period to be sure it has a chance to refresh
        std::thread::sleep(refresh_period * 2);

        // Verify that the registry's contents were updated
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    /// Verifies that when forced refresh feature is disabled, the registry is not refreshed on
    /// read.
    #[test]
    fn forced_refresh_disabled() {
        let test_config = TestConfig::setup("forced_refresh_disabled", Some(mock_registry()));

        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        verify_internal_cache(&test_config, &remote_registry, mock_registry());

        test_config.update_registry(Some(vec![]));

        // Verify that the registry's contents are the same as before, even though the remote file
        // was updated
        verify_internal_cache(&test_config, &remote_registry, mock_registry());

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    /// Verifies that when forced refresh is turned on, the registry refreshes on read after the
    /// refresh period has elapsed.
    #[test]
    fn forced_refresh_enabled() {
        let test_config = TestConfig::setup("forced_refresh_enabled", Some(mock_registry()));

        let refresh_period = Duration::from_millis(10);
        let remote_registry = RemoteYamlRegistry::new(
            test_config.url(),
            test_config.path(),
            None,
            Some(refresh_period),
        )
        .expect("Failed to create registry");

        verify_internal_cache(&test_config, &remote_registry, mock_registry());

        test_config.update_registry(Some(vec![]));

        // Wait at least as long as the forced refresh period
        std::thread::sleep(refresh_period);

        // Verify that the registry's contents are updated on read
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    /// Verifies that any changes made to the remote file are fetched on restart if the remote file
    /// is available.
    #[test]
    fn restart_file_available() {
        let test_config = TestConfig::setup("restart_file_available", Some(mock_registry()));

        // Start the registry the first time, verify its contents, and shut it down
        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");
        verify_internal_cache(&test_config, &remote_registry, mock_registry());
        remote_registry.shutdown_handle().shutdown();

        // Update the remote file
        test_config.update_registry(Some(vec![]));

        // Start the registry again and verify that it has the updated registry contents
        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    /// Verifies that if the remote file is not available when the registry restarts, the old
    /// contents will still be available.
    #[test]
    fn restart_file_unavailable() {
        let test_config = TestConfig::setup("restart_file_unavailable", Some(mock_registry()));

        // Start the registry the first time, verify its contents, and shut it down
        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");
        verify_internal_cache(&test_config, &remote_registry, mock_registry());
        remote_registry.shutdown_handle().shutdown();

        // Make the remote file unavailable
        test_config.update_registry(None);

        // Start the registry again and verify that the old contents are still available
        let remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");
        verify_internal_cache(&test_config, &remote_registry, mock_registry());

        remote_registry.shutdown_handle().shutdown();
        test_config.shutdown();
    }

    // Restart, remote file not available

    /// Creates a mock registry.
    fn mock_registry() -> Vec<Node> {
        vec![
            Node::builder("Node-123")
                .with_endpoint("tcps://12.0.0.123:8431")
                .with_display_name("Bitwise IO - Node 1")
                .with_key("abcd")
                .with_metadata("company", "Bitwise IO")
                .with_metadata("admin", "Bob")
                .build()
                .expect("Failed to build node1"),
            Node::builder("Node-456")
                .with_endpoint("tcps://12.0.0.123:8434")
                .with_display_name("Cargill - Node 1")
                .with_key("0123")
                .with_metadata("company", "Cargill")
                .with_metadata("admin", "Carol")
                .build()
                .expect("Failed to build node2"),
            Node::builder("Node-789")
                .with_endpoint("tcps://12.0.0.123:8435")
                .with_display_name("Cargill - Node 2")
                .with_key("4567")
                .with_metadata("company", "Cargill")
                .with_metadata("admin", "Charlie")
                .build()
                .expect("Failed to build node3"),
        ]
    }

    /// Verifies that the retrieved nodes and the backing file of the `remote_registry` match the
    /// contents of the `expected_registry`.
    fn verify_internal_cache(
        test_config: &TestConfig,
        remote_registry: &RemoteYamlRegistry,
        expected_registry: Vec<Node>,
    ) {
        // Verify the internal list of nodes
        assert_eq!(
            remote_registry.get_nodes().expect("Failed to get nodes"),
            expected_registry,
        );

        // Verify the backing file's contents
        let filename = compute_cache_filename(test_config.url(), test_config.path())
            .expect("Failed to compute cache filename");
        let file = File::open(filename).expect("Failed to open cache file");
        let file_contents: Vec<Node> =
            serde_yaml::from_reader(file).expect("Failed to deserialize cache file");
        assert_eq!(file_contents, expected_registry);
    }

    /// Simplifies tests by handling some of the setup and tear down.
    struct TestConfig {
        _temp_dir: TempDir,
        temp_dir_path: String,
        registry: Arc<Mutex<Option<Vec<Node>>>>,
        registry_url: String,
        rest_api_shutdown_handle: RestApiShutdownHandle,
        rest_api_join_handle: std::thread::JoinHandle<()>,
    }

    impl TestConfig {
        /// Setup for the test, using the `test_name` as the prefix for the temp directory and the
        /// `registry` to populate the remote file (if `Some`, otherwise the remote file won't be
        /// available).
        fn setup(test_name: &str, registry: Option<Vec<Node>>) -> Self {
            let temp_dir = TempDir::new(test_name).expect("Failed to create temp dir");
            let temp_dir_path = temp_dir
                .path()
                .to_str()
                .expect("Failed to get path")
                .to_string();

            let registry = Arc::new(Mutex::new(registry));

            let (rest_api_shutdown_handle, rest_api_join_handle, registry_url) =
                serve_registry(registry.clone());

            Self {
                _temp_dir: temp_dir,
                temp_dir_path,
                registry,
                registry_url,
                rest_api_shutdown_handle,
                rest_api_join_handle,
            }
        }

        /// Gets the temp directory's path
        fn path(&self) -> &str {
            &self.temp_dir_path
        }

        /// Gets the URL for the registry file
        fn url(&self) -> &str {
            &self.registry_url
        }

        /// Updates the `registry` file served up by the REST API; if `registry` is `None`, the
        /// remote file won't be available.
        fn update_registry(&self, registry: Option<Vec<Node>>) {
            *self.registry.lock().expect("Registry lock poisonsed") = registry;
        }

        /// Shuts down the REST API; this should be called at the end of every test that uses
        /// `TestConfig`.
        fn shutdown(self) {
            self.rest_api_shutdown_handle
                .shutdown()
                .expect("Unable to shutdown rest api");
            self.rest_api_join_handle
                .join()
                .expect("Unable to join rest api thread");
        }
    }

    /// Wraps `run_rest_api_on_open_port`, serving up the given `registry` as a registry YAML file
    /// that can be fetched at the returned URL. If `registry` is `None`, the registry file will not
    /// be available.
    fn serve_registry(
        registry: Arc<Mutex<Option<Vec<Node>>>>,
    ) -> (RestApiShutdownHandle, std::thread::JoinHandle<()>, String) {
        let (shutdown, join, url) = run_rest_api_on_open_port(vec![Resource::build(
            "/registry.yaml",
        )
        .add_method(Method::Get, move |_, _| {
            Box::new(match &*registry.lock().expect("Registry lock poisoned") {
                Some(registry) => HttpResponse::Ok()
                    .body(serde_yaml::to_vec(&registry).expect("Failed to serialize registry file"))
                    .into_future(),
                None => HttpResponse::NotFound().finish().into_future(),
            })
        })]);

        (shutdown, join, format!("http://{}/registry.yaml", url))
    }

    /// Runs a REST API with the given `resources` on an open port. Returned string is the URL the
    /// REST API is bound to.
    fn run_rest_api_on_open_port(
        resources: Vec<Resource>,
    ) -> (RestApiShutdownHandle, std::thread::JoinHandle<()>, String) {
        (10000..20000)
            .find_map(|port| {
                let bind_url = format!("127.0.0.1:{}", port);
                let result = RestApiBuilder::new()
                    .with_bind(&bind_url)
                    .add_resources(resources.clone())
                    .build_insecure()
                    .expect("Failed to build REST API")
                    .run_insecure();
                match result {
                    Ok((shutdown_handle, join_handle)) => {
                        Some((shutdown_handle, join_handle, bind_url))
                    }
                    Err(RestApiServerError::BindError(_)) => None,
                    Err(err) => panic!("Failed to run REST API: {}", err),
                }
            })
            .expect("No port available")
    }
}
