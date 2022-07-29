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

//! This module defines the REST API endpoints for interacting with registries.

mod error;
mod nodes;
mod nodes_identity;
mod resources;

use crate::framework::{Resource, RestResourceProvider};
#[cfg(feature = "authorization")]
use splinter_rest_api_common::auth::Permission;

use splinter::registry::RwRegistry;

#[cfg(feature = "authorization")]
const REGISTRY_READ_PERMISSION: Permission = Permission::Check {
    permission_id: "registry.read",
    permission_display_name: "Registry read",
    permission_description: "Allows the client to read the registry",
};
#[cfg(feature = "authorization")]
const REGISTRY_WRITE_PERMISSION: Permission = Permission::Check {
    permission_id: "registry.write",
    permission_display_name: "Registry write",
    permission_description: "Allows the client to modify the registry",
};

pub struct RwRegistryRestResourceProvider {
    resources: Vec<Resource>,
}

impl RwRegistryRestResourceProvider {
    pub fn new(registry: &dyn RwRegistry) -> Self {
        let resources = vec![
            nodes_identity::make_nodes_identity_resource(registry.clone_box()),
            nodes::make_nodes_resource(registry.clone_box()),
        ];
        Self { resources }
    }
}

/// The `RwRegistryRestResourceProvider` struct provides the following endpoints
/// as REST API resources:
///
/// * `GET /registry/nodes` - List the nodes in the registry
/// * `POST /registry/nodes` - Add a node to the registry
/// * `GET /registry/nodes/{identity}` - Fetch a specific node in the registry
/// * `PUT /registry/nodes/{identity}` - Replace a node in the registry
/// * `DELETE /registry/nodes/{identity}` - Delete a node from the registry
impl RestResourceProvider for RwRegistryRestResourceProvider {
    fn resources(&self) -> Vec<Resource> {
        self.resources.clone()
    }
}

#[cfg(feature = "registry-remote")]
#[cfg(test)]
mod tests {
    use super::*;

    use std::fs::File;

    use actix_web::HttpResponse;
    use futures::future::IntoFuture;
    use tempfile::{Builder, TempDir};

    use splinter::registry::RemoteYamlRegistry;
    #[cfg(feature = "authorization")]
    use splinter_rest_api_common::auth::Permission;

    use crate::framework::{Method, Resource, RestApiBuilder, RestApiShutdownHandle};

    /// Verifies that a remote file that contains two nodes with the same identity is rejected (not
    /// loaded).
    #[test]
    fn duplicate_identity() {
        let mut registry = mock_registry();
        registry[0].identity = "identity".into();
        registry[1].identity = "identity".into();
        let test_config = TestConfig::setup("duplicate_identity", Some(registry));

        let mut remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        // Verify that the registry is still empty
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
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

        let mut remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        // Verify that the registry is still empty
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
        test_config.shutdown();
    }

    /// Verifies that a remote file that contains a node with an empty string as its identity is
    /// rejected (not loaded).
    #[test]
    fn empty_identity() {
        let mut registry = mock_registry();
        registry[0].identity = "".into();
        let test_config = TestConfig::setup("empty_identity", Some(registry));

        let mut remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        // Verify that the registry is still empty
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
        test_config.shutdown();
    }

    /// Verifies that a remote file that contains a node with an empty string in its endpoints is
    /// rejected (not loaded).
    #[test]
    fn empty_endpoint() {
        let mut registry = mock_registry();
        registry[0].endpoints = vec!["".into()];
        let test_config = TestConfig::setup("empty_endpoint", Some(registry));

        let mut remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        // Verify that the registry is still empty
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
        test_config.shutdown();
    }

    /// Verifies that a remote file that contains a node with an empty string as its display name is
    /// rejected (not loaded).
    #[test]
    fn empty_display_name() {
        let mut registry = mock_registry();
        registry[0].display_name = "".into();
        let test_config = TestConfig::setup("empty_display_name", Some(registry));

        let mut remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        // Verify that the registry is still empty
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
        test_config.shutdown();
    }

    /// Verifies that a remote file that contains a node with an empty string in its keys is
    /// rejected (not loaded).
    #[test]
    fn empty_key() {
        let mut registry = mock_registry();
        registry[0].keys = vec!["".into()];
        let test_config = TestConfig::setup("empty_key", Some(registry));

        let mut remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        // Verify that the registry is still empty
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
        test_config.shutdown();
    }

    /// Verifies that a remote file that contains a node with no endpoints is rejected (not loaded).
    #[test]
    fn missing_endpoints() {
        let mut registry = mock_registry();
        registry[0].endpoints = vec![];
        let test_config = TestConfig::setup("missing_endpoints", Some(registry));

        let mut remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        // Verify that the registry is still empty
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
        test_config.shutdown();
    }

    /// Verifies that a remote file that contains a node with no keys is rejected (not loaded).
    #[test]
    fn missing_keys() {
        let mut registry = mock_registry();
        registry[0].keys = vec![];
        let test_config = TestConfig::setup("missing_keys", Some(registry));

        let mut remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        // Verify that the registry is still empty
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
        test_config.shutdown();
    }

    /// Verifies that `fetch_node` with an existing identity returns the correct node.
    #[test]
    fn fetch_node_ok() {
        let test_config = TestConfig::setup("fetch_node_ok", Some(mock_registry()));

        let mut remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        let expected_node = mock_registry().pop().expect("Failed to get expected node");
        let node = remote_registry
            .get_node(&expected_node.identity)
            .expect("Failed to fetch node")
            .expect("Node not found");
        assert_eq!(node, expected_node);

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
        test_config.shutdown();
    }

    /// Verifies that `fetch_node` with a non-existent identity returns Ok(None)
    #[test]
    fn fetch_node_not_found() {
        let test_config = TestConfig::setup("fetch_node_not_found", Some(mock_registry()));

        let mut remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        assert!(remote_registry
            .get_node("NodeNotInRegistry")
            .expect("Failed to fetch node")
            .is_none());

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
        test_config.shutdown();
    }

    ///
    /// Verifies that `has_node` properly determines if a node exists in the registry.
    ///
    #[test]
    fn has_node() {
        let test_config = TestConfig::setup("has_node", Some(mock_registry()));

        let mut remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        let expected_node = mock_registry().pop().expect("Failed to get expected node");
        assert!(remote_registry
            .has_node(&expected_node.identity)
            .expect("Failed to check if expected_node exists"));
        assert!(!remote_registry
            .has_node("NodeNotInRegistry")
            .expect("Failed to check for non-existent node"));

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
        test_config.shutdown();
    }

    /// Verifies that `list_nodes` returns all nodes in the remote file.
    #[test]
    fn list_nodes() {
        let test_config = TestConfig::setup("list_nodes", Some(mock_registry()));

        let mut remote_registry =
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

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
        test_config.shutdown();
    }

    /// Verifies that `list_nodes` returns an empty list when there are no nodes in the remote file.
    #[test]
    fn list_nodes_empty() {
        let test_config = TestConfig::setup("list_nodes_empty", Some(vec![]));

        let mut remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        let nodes = remote_registry
            .list_nodes(&[])
            .expect("Failed to retrieve nodes")
            .collect::<Vec<_>>();

        assert!(nodes.is_empty());

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
        test_config.shutdown();
    }

    /// Verifies that `list_nodes` returns the correct nodes when a metadata filter is provided.
    #[test]
    fn list_nodes_filter_metadata() {
        let test_config = TestConfig::setup("list_nodes_filter_metadata", Some(mock_registry()));

        let mut remote_registry =
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

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
        test_config.shutdown();
    }

    /// Verifies that `list_nodes` returns the correct nodes when multiple metadata filters are
    /// provided.
    #[test]
    fn list_nodes_filter_multiple() {
        let test_config = TestConfig::setup("list_nodes_filter_multiple", Some(mock_registry()));

        let mut remote_registry =
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

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
        test_config.shutdown();
    }

    /// Verifies that `list_nodes` returns an empty list when no nodes fit the filtering criteria.
    #[test]
    fn list_nodes_filter_empty() {
        let test_config = TestConfig::setup("list_nodes_filter_empty", Some(mock_registry()));

        let mut remote_registry =
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

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
        test_config.shutdown();
    }

    /// Verifies that when the remote file is available at startup, it's fetched and cached
    /// successfully. The internal list of nodes and the backing file should match the remote file.
    #[test]
    fn file_available_at_startup() {
        let test_config = TestConfig::setup("file_available_at_startup", Some(mock_registry()));

        let mut remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        verify_internal_cache(&test_config, &remote_registry, mock_registry());

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
        test_config.shutdown();
    }

    /// Verifies that when the remote file is not available at startup, the registry starts up with
    /// an empty cache. When the remote file becomes available, it should be fetched and cached on
    /// the next read.
    #[test]
    fn file_unavailable_at_startup() {
        // Start without a remote file
        let test_config = TestConfig::setup("file_unavailable_at_startup", None);

        let mut remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        // Verify that the registry is still empty
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        // Make the remote file available now
        test_config.update_registry(Some(mock_registry()));

        // Verify that the registry's contents were updated
        verify_internal_cache(&test_config, &remote_registry, mock_registry());

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
        test_config.shutdown();
    }

    /// Verifies that when auto refresh is turned off, the auto refresh thread is not running.
    #[test]
    fn auto_refresh_disabled() {
        let test_config = TestConfig::setup("auto_refresh_disabled", Some(mock_registry()));

        let mut remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");

        // The `running` atomic bool is only set if the auto refresh thread was started.
        assert!(shutdown_handle.running.is_none());

        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
        test_config.shutdown();
    }

    /// Verifies that when auto refresh is turned on, the auto refresh thread is running and
    /// refreshes the registry in the background
    #[test]
    fn auto_refresh_enabled() {
        let test_config = TestConfig::setup("auto_refresh_enabled", Some(mock_registry()));

        let refresh_period = Duration::from_secs(1);
        let mut remote_registry = RemoteYamlRegistry::new(
            test_config.url(),
            test_config.path(),
            Some(refresh_period),
            None,
        )
        .expect("Failed to create registry");

        verify_internal_cache(&test_config, &remote_registry, mock_registry());

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");

        // The `running` atomic bool is only set if the auto refresh thread was started.
        assert!(shutdown_handle.running.is_some());

        test_config.update_registry(Some(vec![]));

        // Wait twice as long as the auto refresh period to be sure it has a chance to refresh
        std::thread::sleep(refresh_period * 2);

        // Verify that the registry's contents were updated
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
        test_config.shutdown();
    }

    /// Verifies that when forced refresh feature is disabled, the registry is not refreshed on
    /// read.
    #[test]
    fn forced_refresh_disabled() {
        let test_config = TestConfig::setup("forced_refresh_disabled", Some(mock_registry()));

        let mut remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");

        verify_internal_cache(&test_config, &remote_registry, mock_registry());

        test_config.update_registry(Some(vec![]));

        // Verify that the registry's contents are the same as before, even though the remote file
        // was updated
        verify_internal_cache(&test_config, &remote_registry, mock_registry());

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
        test_config.shutdown();
    }

    /// Verifies that when forced refresh is turned on, the registry refreshes on read after the
    /// refresh period has elapsed.
    #[test]
    fn forced_refresh_enabled() {
        let test_config = TestConfig::setup("forced_refresh_enabled", Some(mock_registry()));

        let refresh_period = Duration::from_millis(10);
        let mut remote_registry = RemoteYamlRegistry::new(
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

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
        test_config.shutdown();
    }

    /// Verifies that any changes made to the remote file are fetched on restart if the remote file
    /// is available.
    #[test]
    fn restart_file_available() {
        let test_config = TestConfig::setup("restart_file_available", Some(mock_registry()));

        // Start the registry the first time, verify its contents, and shut it down
        let mut remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");
        verify_internal_cache(&test_config, &remote_registry, mock_registry());

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");

        // Update the remote file
        test_config.update_registry(Some(vec![]));

        // Start the registry again and verify that it has the updated registry contents
        let mut remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");
        verify_internal_cache(&test_config, &remote_registry, vec![]);

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
        test_config.shutdown();
    }

    /// Verifies that if the remote file is not available when the registry restarts, the old
    /// contents will still be available.
    #[test]
    fn restart_file_unavailable() {
        let test_config = TestConfig::setup("restart_file_unavailable", Some(mock_registry()));

        // Start the registry the first time, verify its contents, and shut it down
        let mut remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");
        verify_internal_cache(&test_config, &remote_registry, mock_registry());
        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");

        // Make the remote file unavailable
        test_config.update_registry(None);

        // Start the registry again and verify that the old contents are still available
        let mut remote_registry =
            RemoteYamlRegistry::new(test_config.url(), test_config.path(), None, None)
                .expect("Failed to create registry");
        verify_internal_cache(&test_config, &remote_registry, mock_registry());

        let mut shutdown_handle = remote_registry
            .take_shutdown_handle()
            .expect("Unable to get shutdown handle");
        shutdown_handle.signal_shutdown();
        shutdown_handle
            .wait_for_shutdown()
            .expect("Unable to shutdown remote registry");
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
        let file_contents: Vec<YamlNode> =
            serde_yaml::from_reader(file).expect("Failed to deserialize cache file");

        let file_contents_nodes: Vec<Node> = file_contents
            .into_iter()
            .map(|node| Node::try_from(node).expect("Unable to build node"))
            .collect();
        assert_eq!(file_contents_nodes, expected_registry);
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
            let temp_dir = Builder::new()
                .prefix(test_name)
                .tempdir()
                .expect("Failed to create temp dir");
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
        let mut resource = Resource::build("/registry.yaml");
        #[cfg(feature = "authorization")]
        {
            resource = resource.add_method(
                Method::Get,
                Permission::AllowUnauthenticated,
                move |_, _| {
                    Box::new(match &*registry.lock().expect("Registry lock poisoned") {
                        Some(registry) => {
                            let yaml_registry: Vec<YamlNode> = registry
                                .iter()
                                .map(|node| YamlNode::from(node.clone()))
                                .collect();
                            HttpResponse::Ok()
                                .body(
                                    serde_yaml::to_vec(&yaml_registry)
                                        .expect("Failed to serialize registry file"),
                                )
                                .into_future()
                        }
                        None => HttpResponse::NotFound().finish().into_future(),
                    })
                },
            )
        }
        #[cfg(not(feature = "authorization"))]
        {
            resource = resource.add_method(Method::Get, move |_, _| {
                Box::new(match &*registry.lock().expect("Registry lock poisoned") {
                    Some(registry) => {
                        let yaml_registry: Vec<YamlNode> = registry
                            .iter()
                            .map(|node| YamlNode::from(node.clone()))
                            .collect();
                        HttpResponse::Ok()
                            .body(
                                serde_yaml::to_vec(&yaml_registry)
                                    .expect("Failed to serialize registry file"),
                            )
                            .into_future()
                    }
                    None => HttpResponse::NotFound().finish().into_future(),
                })
            })
        }
        let (shutdown, join, url) = run_rest_api_on_open_port(vec![resource]);

        (shutdown, join, format!("http://{}/registry.yaml", url))
    }

    fn run_rest_api_on_open_port(
        resources: Vec<Resource>,
    ) -> (RestApiShutdownHandle, std::thread::JoinHandle<()>, String) {
        #[cfg(not(feature = "https-bind"))]
        let bind = "127.0.0.1:0";
        #[cfg(feature = "https-bind")]
        let bind = crate::rest_api::BindConfig::Http("127.0.0.1:0".into());

        let result = RestApiBuilder::new()
            .with_bind(bind)
            .add_resources(resources.clone())
            .build_insecure()
            .expect("Failed to build REST API")
            .run_insecure();
        match result {
            Ok((shutdown_handle, join_handle)) => {
                let port = shutdown_handle.port_numbers()[0];
                (shutdown_handle, join_handle, format!("127.0.0.1:{}", port))
            }
            Err(err) => panic!("Failed to run REST API: {}", err),
        }
    }
}
