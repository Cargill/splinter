// Copyright 2021 Cargill Incorporated
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

use std::collections::HashMap;

use cylinder::{jwt::JsonWebTokenBuilder, secp256k1::Secp256k1Context, Context, Signer};
use reqwest::blocking::Client;
use serde::Deserialize;
use splinter::admin::client::event::EventType;
use splinter::admin::messages::AuthorizationType;
use splinter::biome::profile::store::ProfileBuilder;
use splinter::error::InternalError;
use splinterd::node::Node;
use splinterd::node::PermissionConfig;
use splinterd::node::RestApiVariant;

use crate::admin::circuit_commit::commit_2_party_circuit_with_auth;
use crate::admin::get_node_service_id_with_auth;
use crate::admin::payload::make_create_circuit_payload;
use crate::framework::network::Network;

/// Test that if no permissions are configured, all REST API endpoints that require permission
/// will return a 401 unauthorized if attempted to access
///
/// 1. Get a hashmap of all rest api endpoints provided by the testing framework that require
///    permissions to be accessed
/// 2. Create a permission config with the `circuit.read` and `circuit.write` permissions so that
///    a circuit can be created
/// 3. Create a network with two nodes, using `with_permission_config` and passing in the permission
///    config created in the last step
/// 4. Create a 2 party circuit
/// 5. Attempt to access each endpoint
/// 6. Check that a 401 'Client is not authorized' is returned
#[test]
fn test_endpoints_with_no_permissions() {
    // Get all endpoints and their methods and permissions
    let endpoint_perm_map = create_endpoint_permission_map();

    // Create a separate `PermissionConfig` with the necessary permissions to create a circuit
    let circuit_create_perm_config = PermissionConfig::new(
        vec!["circuit.write".into(), "circuit.read".into()],
        new_signer(),
    );
    let signer = &*circuit_create_perm_config.signer();

    // Start a 2-node network with the only `PermissionConfig` being the one for the signer
    // that will be used to create the circuit. The `PermissionConfig`s that grant signers
    // permission to access each of the REST API endpoints are not submitted so that each
    // endpoint can be tested by a signer with no configured permissions
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .with_permission_config(vec![circuit_create_perm_config])
        .with_cylinder_auth()
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");
    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");
    // Create the token and auth string for the signer that has permission to submit a circuit
    let token = JsonWebTokenBuilder::new()
        .build(signer)
        .expect("failed to build jwt");

    let auth = &format!("Bearer Cylinder:{}", token);

    let circuit_id = "ABCDE-01234";

    // Create a 2 pary circuit
    commit_2_party_circuit_with_auth(
        circuit_id,
        node_a,
        node_b,
        AuthorizationType::Trust,
        auth.into(),
    );

    let service_id_a = get_node_service_id_with_auth(&circuit_id, node_a, auth.into());

    // Loop through all endpoints checking that a "401 unauthorized" is returned for each
    for (endpoint, methods) in endpoint_perm_map {
        let endpoint = endpoint.replace("SERVICE_ID", service_id_a.service_id());
        let endpoint = endpoint.replace("NODE_ID", node_a.node_id());
        let url = format!("http://localhost:{}{}", &node_a.rest_api_port(), endpoint);
        assert!(check_endpoint_no_perm(methods, url,).is_ok());
    }

    shutdown!(network).expect("Unable to shutdown network");
}

// Send requests to the given endpoint and check that a 401 is returned each time
fn check_endpoint_no_perm(
    methods: Vec<(String, PermissionConfig)>,
    endpoint_url: String,
) -> Result<(), InternalError> {
    for (method, perm_config) in methods {
        // Create a jwt for the associated signer in the `PermissionConfig`
        let token = JsonWebTokenBuilder::new()
            .build(&*perm_config.signer())
            .expect("failed to build jwt");
        let auth = format!("Bearer Cylinder:{}", token);
        // Send a request to the specified endpoint
        let res = match method.as_ref() {
            "get" => Client::new()
                .get(&endpoint_url)
                .header("Authorization", auth.clone())
                .send()
                .map_err(|err| InternalError::from_source(Box::new(err))),
            "post" => Client::new()
                .post(&endpoint_url)
                .header("Authorization", auth.clone())
                .send()
                .map_err(|err| InternalError::from_source(Box::new(err))),
            "put" => Client::new()
                .put(&endpoint_url)
                .header("Authorization", auth.clone())
                .send()
                .map_err(|err| InternalError::from_source(Box::new(err))),
            "patch" => Client::new()
                .patch(&endpoint_url)
                .header("Authorization", auth.clone())
                .send()
                .map_err(|err| InternalError::from_source(Box::new(err))),
            "delete" => Client::new()
                .delete(&endpoint_url)
                .header("Authorization", auth.clone())
                .send()
                .map_err(|err| InternalError::from_source(Box::new(err))),
            _ => panic!("shouldn't reach here"),
        };
        res.and_then(|res| {
            let status = res.status();
            if status.as_u16() == 401 {
                Ok(())
            } else {
                let message = res
                    .json::<ServerError>()
                    .map_err(|_| {
                        InternalError::with_message(format!(
                            "Request failed with status code '{}', but error \
                            response was not valid",
                            status
                        ))
                    })?
                    .message;

                return Err(InternalError::with_message(format!(
                    "Got a response other than 401 from endpoint {}: {}",
                    endpoint_url, message
                )));
            }
        })?;
    }

    Ok(())
}

// Creates a hashmap that maps the REST API endpoints to a vector of the available methods and
// a corresponding `PermissionConfig`s that contain the required permission for the method and a
// signer
fn create_endpoint_permission_map() -> HashMap<String, Vec<(String, PermissionConfig)>> {
    let mut endpoints: HashMap<String, Vec<(String, PermissionConfig)>> = HashMap::new();

    endpoints.insert(
        "/admin/proposals".into(),
        vec![(
            "get".into(),
            PermissionConfig::new(vec!["circuit.read".into()], new_signer()),
        )],
    );
    endpoints.insert(
        "/admin/proposals/ABCDE-56789".into(),
        vec![(
            "get".into(),
            PermissionConfig::new(vec!["circuit.read".into()], new_signer()),
        )],
    );
    endpoints.insert(
        "/admin/submit".into(),
        vec![(
            "post".into(),
            PermissionConfig::new(vec!["circuit.write".into()], new_signer()),
        )],
    );
    endpoints.insert(
        format!("/ws/admin/register/type").into(),
        vec![(
            "get".into(),
            PermissionConfig::new(vec!["circuit.read".into()], new_signer()),
        )],
    );
    endpoints.insert(
        "/admin/circuits".into(),
        vec![(
            "get".into(),
            PermissionConfig::new(vec!["circuit.read".into()], new_signer()),
        )],
    );
    endpoints.insert(
        "/admin/circuits/ABCDE-01234".into(),
        vec![(
            "get".into(),
            PermissionConfig::new(vec!["circuit.read".into()], new_signer()),
        )],
    );
    endpoints.insert(
        "/authorization/maintenance".into(),
        vec![
            (
                "get".into(),
                PermissionConfig::new(vec!["authorization.maintenance.read".into()], new_signer()),
            ),
            (
                "post".into(),
                PermissionConfig::new(vec!["authorization.maintenance.write".into()], new_signer()),
            ),
        ],
    );
    endpoints.insert(
        "/authorization/permissions".into(),
        vec![(
            "get".into(),
            PermissionConfig::new(vec!["authorization.permissions.read".into()], new_signer()),
        )],
    );
    endpoints.insert(
        "/authorization/roles".into(),
        vec![
            (
                "get".into(),
                PermissionConfig::new(vec!["authorization.rbac.read".into()], new_signer()),
            ),
            (
                "post".into(),
                PermissionConfig::new(vec!["authorization.rbac.write".into()], new_signer()),
            ),
        ],
    );
    endpoints.insert(
        format!("/authorization/roles/11").into(),
        vec![(
            "get".into(),
            PermissionConfig::new(vec!["authorization.rbac.read".into()], new_signer()),
        )],
    );
    endpoints.insert(
        "/registry/nodes".into(),
        vec![
            (
                "post".into(),
                PermissionConfig::new(vec!["registry.write".into()], new_signer()),
            ),
            (
                "get".into(),
                PermissionConfig::new(vec!["registry.read".into()], new_signer()),
            ),
        ],
    );
    endpoints.insert(
        "/registry/nodes/NODE_ID".into(),
        vec![
            (
                "get".into(),
                PermissionConfig::new(vec!["registry.read".into()], new_signer()),
            ),
            (
                "put".into(),
                PermissionConfig::new(vec!["registry.write".into()], new_signer()),
            ),
            (
                "delete".into(),
                PermissionConfig::new(vec!["registry.write".into()], new_signer()),
            ),
        ],
    );
    endpoints.insert(
        "/scabbard/ABCDE-01234/SERVICE_ID/batches".into(),
        vec![(
            "post".into(),
            PermissionConfig::new(vec!["scabbard.write".into()], new_signer()),
        )],
    );
    endpoints.insert(
        "/scabbard/ABCDE-01234/SERVICE_ID/batch_statuses?ids=6ff35474a572087e08fd6a54d563bd8172951b363e5c9731f1a40a855e14bba45dac515364a08d8403f4fb5d4a206174b7f63c29e4f4e425dc71b95494b8a798".into(),
        vec![(
            "get".into(),
            PermissionConfig::new(vec!["scabbard.read".into()], new_signer()),
        )],
    );
    endpoints.insert(
        "/scabbard/ABCDE-01234/SERVICE_ID/state".into(),
        vec![(
            "get".into(),
            PermissionConfig::new(vec!["scabbard.read".into()], new_signer()),
        )],
    );
    endpoints.insert(
        "/scabbard/ABCDE-01234/SERVICE_ID/state/00ec01b114f311db0e009ca2a88a9b97b1d7b362ddb27dc3dd214c6d20327a1fc3add8".into(),
        vec![(
            "get".into(),
            PermissionConfig::new(vec!["scabbard.read".into()], new_signer()),
        )],
    );
    endpoints.insert(
        "/biome/users".into(),
        vec![(
            "get".into(),
            PermissionConfig::new(vec!["biome.user.read".into()], new_signer()),
        )],
    );
    endpoints.insert(
        "/biome/users/USER_ID".into(),
        vec![
            (
                "get".into(),
                PermissionConfig::new(vec!["biome.user.read".into()], new_signer()),
            ),
            (
                "put".into(),
                PermissionConfig::new(vec!["biome.user.write".into()], new_signer()),
            ),
            (
                "delete".into(),
                PermissionConfig::new(vec!["biome.user.write".into()], new_signer()),
            ),
        ],
    );
    endpoints.insert(
        "/biome/profiles".into(),
        vec![(
            "get".into(),
            PermissionConfig::new(vec!["biome.profile.read".into()], new_signer()),
        )],
    );
    endpoints.insert(
        "/biome/profiles/test_user_id".into(),
        vec![(
            "get".into(),
            PermissionConfig::new(vec!["biome.profile.read".into()], new_signer()),
        )],
    );
    endpoints
}

// Creates and submits a new circuit proposal
fn create_circuit_proposal(circuit_id: &str, node_a: &Node, node_b: &Node, auth: &str) {
    // Create the list of node details needed to build the `CircuitCreateRequest`
    let node_info = vec![
        (
            node_a.node_id().to_string(),
            (
                node_a.network_endpoints().to_vec(),
                // get the second signer (not the normal key in the first position)
                node_a
                    .signers()
                    .get(1)
                    .expect("node does not have enough signers configured")
                    .public_key()
                    .expect("Unable to get first node's public key"),
            ),
        ),
        (
            node_b.node_id().to_string(),
            (
                node_b.network_endpoints().to_vec(),
                node_b
                    .signers()
                    .get(0)
                    .expect("node does not have enough signers configured")
                    .public_key()
                    .expect("Unable to get first node's public key"),
            ),
        ),
    ]
    .into_iter()
    .collect::<HashMap<String, (Vec<String>, cylinder::PublicKey)>>();

    let node_a_event_client = node_a
        .admin_service_event_client_with_auth(
            &format!("test_circuit_{}", &circuit_id),
            None,
            auth.into(),
        )
        .expect("Unable to get event client");
    let node_b_event_client = node_b
        .admin_service_event_client_with_auth(
            &format!("test_circuit_{}", &circuit_id),
            None,
            auth.into(),
        )
        .expect("Unable to get event client");

    let circuit_payload_bytes = make_create_circuit_payload(
        &circuit_id,
        node_a.node_id(),
        node_info,
        &*node_a.admin_signer().clone_box(),
        &vec![
            node_a
                .admin_signer()
                .public_key()
                .expect("Unable to get first node's public key")
                .as_hex(),
            node_b
                .admin_signer()
                .public_key()
                .expect("Unable to get second node's public key")
                .as_hex(),
        ],
        AuthorizationType::Challenge,
    )
    .expect("Unable to generate circuit request");
    // Submit the `CircuitManagementPayload` to the first node
    let res = node_a
        .admin_service_client_with_auth(auth.into())
        .submit_admin_payload(circuit_payload_bytes.clone());
    assert!(res.is_ok());

    // Wait for the proposal event from each node.
    let proposal_a_event = node_a_event_client
        .next_event()
        .expect("Unable to get next event");
    let proposal_b_event = node_b_event_client
        .next_event()
        .expect("Unable to get next event");

    assert_eq!(&EventType::ProposalSubmitted, proposal_a_event.event_type());
    assert_eq!(&EventType::ProposalSubmitted, proposal_b_event.event_type());
    assert_eq!(proposal_a_event.proposal(), proposal_b_event.proposal());
}

fn new_signer() -> Box<dyn Signer> {
    let context = Secp256k1Context::new();
    context.new_signer(context.new_random_private_key())
}

#[derive(Deserialize)]
pub struct ServerError {
    pub message: String,
}

// Adds a test profile to the user profile store
fn add_profile(node: &Node) -> Result<(), InternalError> {
    let profile = ProfileBuilder::new()
        .with_user_id("test_user_id".into())
        .with_subject("subject".into())
        .with_name(Some("name".into()))
        .build()
        .expect("Unable to build profile");

    let profile_store = node.user_profile_store();
    Ok(profile_store
        .add_profile(profile)
        .map_err(|err| InternalError::from_source(Box::new(err)))?)
}
