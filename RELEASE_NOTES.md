# Release Notes

## Changes in splinter 0.6.5

### scabbard

* Update sawtooth dependency version to 0.7.3 which includes a new migration to
  add an index for the `transaction_receipt` table, which mitigates a slow-down
  of processing after a large number of transactions.

## Changes in splinter 0.6.4

### splinterd

* Add `sawtooth_sabre` and `sabre_sdk` to the default loggers that include trace
logging as a possible level. This fixes an issue where sabre logs were not
available in the debug output without modifying the log configuration in a
splinter daemon's configuration file.

## Changes in splinter 0.6.3

### scabbard

* Update transact dependency to 0.4.3. This update fixes an issue where
  Merkle-Radix addresses with leading zeros would be stored incorrectly in
  SQLite.

## Changes in splinter 0.6.2

### libsplinter

* Correct rustdoc code links. This change corrects several rustdoc code links
  from one struct to another in different modules, where the linked-struct is
  not explicitly in a use statement.

* Remove lock macros from documentation. These macros should not be public
  members of the library, and may be removed in the future. Mark the doc for
  these as "hidden" in order to remove them.

* Add readme to root module rustdoc.This provides a more informative landing
  documentation landing experience.

* Add missing simple summaries for top-level mods. These fill out the module
  summaries on the main root documentation page for splinter.

* Remove SQLite PRAGMA settings related to WAL journal mode, effectively
  reverting these settings such that the values will be set to the default
  values based on compile-time settings.

* Add sqlite migration for renamed `role` tables. When the roles table was
  updated to be named `rbac_roles` the foreign key constraint on the
  `role_permission` table was not updated making it impossible to insert any
  new role:permission relations because the foreign key constraint would fail.

### splinterd

* Fix ability to pass comma separated values for several options to `splinterd`.
  Previously, the values would be treated as one value, instead of multiple.

* Update open API documentation to match the current endpoints. Includes several
  bug fixes and adding missing endpoints and components

### scabbard

* Remove `ScabbardReceiptStore` types. Remove unnecessary `ScabbardReceiptStore`
  types. This type was initially created so that the receipt store
  implementation used by scabbard could change depending on whether or not the
  `"receipt-store"` experimental feature was enabled. The `"receipt-store"`
  feature has been stabilized and removed so the `ScabbardReceiptStore` type is
  no longer needed.

* Add transact and sawtooth migration checks to any_pending_migrations.

## Changes in splinter 0.6.1

### Highlights

* Add a `state migrate` command for migrating state between database backends.

* Update stores to support write-exclusivity for database backends. This fixes
  a database lock contention issue seen with SQLite deployments.

### libsplinter

* Add validation that no SQL backed trees exist if LMDB is enabled.

* Add `OrchestratableService` to `Orchestrator`.

* Remove lock-contention in `Orchestrator` REST API calls.

* REST API features have been reorganized, placing all Actix Web 1 related code
  under the `"rest-api-actix-web-1"` related feature.

* Several public const values in `crate::protocol` have been moved or made
  private; REST API clients should fill SplinterProtocolVersion to match the
  REST API specification being referenced during development.

* Update stores to support write-exclusivity for database backends.

* The `crate::channel` module which was previously pub is now pub(crate) as it
  is not a useful external to libsplinter and specific channel implementation
  details may change in future releases.

### splinter CLI

* Implement `splinter state migrate` command. This command enables moving
  scabbard state to or from LMDB, deleting from the input database.

### splinterd

* Adjust the default log settings to filter logs at the appender level.

* Change logic behind the `-v` verbosity flag to work better with customized
  log configurations

* Move `"authorization-handler-rbac"` to the `"default"` feature set.

* Update the REST API tests with authorization support.

* Enable exclusive writes for SQLite database connection pools

### libscabbard

* Implement `OrchestratableService` on `ScabbardService`

* Update stores to support write-exclusivity for database backends.

* Update `ScabbardFactory` to include optional write-exclusive SQLite
  configuration.

## Changes in Splinter 0.5.26

### Highlights

* Stabilized the splinterd feature `"log-config"`.  This feature allows for
  finer-grained configuration of the log output.

### splinter CLI

* Add `--group` option to the `splinter keygen` subcommand.  This option allows
  the user to modify the group of the resulting key files, or automatically set
  the group based on the target directory.  The group now defaults to the user's
  group if this option is omitted.

### splinterd

* Stabilize `"log-config"` feature by removing it.  The fine-grained log
  configuration is now available by default.

## Changes in Splinter 0.5.25

### Highlights

* Stabilized the `"scabbard/database-support"`, `"cli/scabbard-migrations"`, and
  `"splinterd/scabbard-database-support"` features. These features now allow all
  scabbard state, including merkle state, to be stored in the database instead
  of being spread across multiple LMDB files. LMDB may be enabled for merkle
  state storage with the optional flag `--scabbard-state lmdb` or by its
  analogous setting in the TOML config file.

### libsplinter

* Save replaced connection ids for peers in the PeerManager so that old
  messages can still be routed.

* Check for inbound unreferenced peer when upgrading an outbound unreferenced
  peer to peers. If one is found, check whether the outbound or inbound
  connection should be used. Fixes an issue where a duplicate connection would
  be kept around.

* Bump the version of jsonwebtoken to 7 to resolve a build issue with the
  dependent ring crate.

* Fix the no-op tap macros such that they can properly accept multiple tags.

### splinter CLI

* Modify the `upgrade_scabbard_receipt_store` function to skip the receipt store
  upgrade process if there are no local circuits found. This fixes a bug where
  running upgrade with no existing circuits would return an error.

* Stabilize `scabbard-migrations` by removing it.

### splinterd

* Change loggers to expect an stdout appender. The default loggers expected a
  "default" named appender, but the name was changed to "stdout".

* Fix a bug in logging where the verbosity flag was altering the root loggers.
  The flag now correctly alters the stdout appenders level.

* Enable `Appender` log level filtering.

* Log all config values after logging configuration so they get recorded in log
  files.

* Fix the `ConnectionPool` feature guards around "database-postgres" and
  "database-sqlite" features, for the case where neither are enabled.

* Simplify the `ByteSize` type and move it into the toml module.

* Stabilize `scabbard-database-support` by removing it.

* Update the root logger to allow levels below `info`. The default root logger
  had a preset level filter of `info`, the value is now `trace` so all logs will
  get forwarded to appenders.

### scabbard

* Replace the boxed function for handling state purge with a trait for the
  specific operation.

* Rename the clap value for `scabbard_storage` to be `scabbard_state`.

* Move the conversion of the TOML version of `ScabbardState` to the config value
  to the `config::toml` module. This ensures the `config::scabbard_state` module
  should not have any dependencies on specific formats.

* Add a circuit_tag to the committed_batches, such that the metrics can be
  correlated for a complete circuit.

* Update transact dependency to min 0.3.13.

* Delete the scabbard SQL merkle state on service purge. Replace the
  `NoOpScabbardPurgeHandler` with DB-specific handlers. These handlers delete
  the entire merkle state tree on purge.

* Move the to_hex function out of the hex module and localize it to the modules
  where it will be used. This removes any feature-guard complexity around the
  function.

* Make the scabbard CLI only depend on the `client-reqwest` feature, removing
  the dependency on the service during compilation time.

* Move the `ScabbardState` parsing to the config::clap module.

* Add missing guards for `lmdb` usage to ensure this functionality is not
  available if the feature is disabled.

* Update all of the factory guards such that the code correctly compiles and
  lints when no database features are enabled.

* Remove the use of scabbard default feature in the CLI.

* Stabilize `database-support` by removing it.

* Collapse the `state_database` and `factory_database` module up into the
  respective root state and factory modules.

* Remove smart permissions from scabbard CLI. Because Smart permissions has been
  removed from the new version of sabre, the unused commands are removed to make
  way for the upgrading to a new version of sabre. Smart permissions were
  experimental so this does not change the stable API.

### gameroom

* Gameroom was setting SplinterProtocolVersion to 1. This is correct for
  gameroom but changes had been made to make gameroom handle v2 versions of
  admin events. Now that admin events version is guarded by protocol version,
  the v1 messages can be used instead.

### build

* Add an OpenAPI lint step to just and GitHub CI to verify that the OpenAPI
  files are formatted correctly and follow best practices.

## Changes in Splinter 0.5.24

### libsplinter

* Verify proposal circuit version is supported for vote. Verify that the circuit
  version for the proposal that is being voted on is still valid for the
  currently support circuit versions.

### splinter CLI

* Create Database file with 640 permissions.

* Add receipt store to the `splinter upgrade` CLI command.

* Check that the value given for the `--rest-api-endpoint` splinterd CLI option
  contains 'http://' or 'https://' and exit with an error if it does not.

### scabbard

* Detect incorrect state daemon. Changes the `ScabbardFactory` builder to fail
  if LMDB state is not enabled, but there are any existing LMDB files in the
  state directory.

## Changes in Splinter 0.5.23

### Highlights

* Stabilized `scabbard-receipt-store` feature in splinter cli.

### libsplinter

* Update admin routes to error if protocol version is too high.

* Replace the use of `Vec<u8>` with `splinter::public_key::PublicKey`. For
  areas that are not on the edges (Rest API/ Database) the structs should use
  `PublicKey` to represent a public key instead of Vec.

* Update admin register route to handle protocol versions. When submitting the
  request the `SplinterProtocolVersion` will determine the version of admin
  events that will be returned over the event subscriber If the protocol is set
  to 1, 0.4 equivalent `AdminEvents` are returned. If the protocol is set to 2
  or not set, 0.5/0.6 `AdminEvent` messages will be returned. This allows for
  backwards compatibility with 0.4 clients to still interact with 0.5 splinter
  nodes.

* Fix update_proposal so it does not remove incorrect nodes. When updating a
  proposal, the command was removing all proposed with the same node ID but
  should remove by circuit ID instead.

* Configure SQLite PRAGMA for multi-threaded support.

* Remove the `create_store_factory` function to push the connection type
  selection decision closer to the caller.

### splinterd

* Add REST API test that checks permissioned endpoints can’t be accessed by a
  client without any set permissions.

* Split connection pool and store factory creation. This change splits the
  creation of the connection pool and the store factory.  It allows the use of
  the connection pool to be passed to the scabbard service as well.

* Add `create_store_factory` function to create stores. This functionality used
  to be in libsplinter.

* Effectively remove the use of the `MemoryStoreFactory`, replacing it with a
  memory-based SQLite version.

* Improve scabbard_state documentation and position in example configuration
  file.

* Remove the `zmq` feature.

* Add checks for log file permissioning issues.

* Add tests for toml configuration file deserialization.

* Add state_dir and config_dir keys to deserializable toml configuration
  structure.

* Add default quiet loggers for tokio, tokio_reactor, and hyper crates.

### libscabbard

* Add checks for invalid merkle state and database configuration.

* LMDB files are written with “.lmdb” file extension.

* Update LMDB default file sizes on 64 bit linux build targets to be 1024^4.

* Switch from use of postgres connection pool to single postgres connection to
  prevent cli looping when a connection could not be established.

### splinter CLI

* Break apart `scabbard-migration` features. The scabbard migrations were only
  being run if the scabbard-migrations feature was enabled which was
  experimental. This means the transaction receipt migrations were not being
  run on stable.

* Switch the use of Pool with PgConnection to just a PgConnection in splinter
  database migrate. This enables testing the connection to the postgres
  database and returning an error if not able to connect. Before with the use
  of Pool, the connection would be retried, causing the CLI command to loop.
  Now it returns if the connection cannot be established.

## Changes in Splinter 0.5.22

### Highlights

* Experimental support for database-backed MerkleState in libscabbard has been
  added.

* Stabilize argument validation for internal services. This enables being able
  to validate arbitrary service arguments in the admin service by defining
  ServiceArgValidator for a service type. The admin service will run the
  validator against a proposed circuit, rejecting a proposal if they are
  invalid.

### libsplinter

* Stabilize `"service-arg-validation"` by removing the feature.

* Fix type for admin service event ID for Postgres. The event ID was being set
  to `INTEGER` but needs to be set to `BIGINT`.

### splinterd

* Stabilize `"service-arg-validation"` by removing the feature.

* Add `scabbard_storage` configuration value. This value will allow splinterd to
  enable the use of LMDB scabbard state.  This value may be configured via
  `splinterd.toml` or the `--scabbard-storage` CLI argument. This is behind the
  experimental feature `"scabbard-database-support"`.

### libscabbard

* Stabilize `"service-arg-validation"` by removing the feature.

* Stabilize `"receipt-store"` by removing the feature.

* Enable the use of dynamic storage in the scabbard service, including the
  `CommitHashStore` and the `ReceiptStore`. This is behind the experimental
  feature `"database-support"`.

* Enable the use of the transact library's `SqlMerkleState` in the scabbard
  service, with LMDB as an optional alternative. This is behind the experimental
  feature `"database-support"`.

* Fix the `dir_path` arg given when creating a new `LmdbReceiptStore`.
  Previously the value given for this argument included the name of the LMDB
  file in the path which caused an error.

* Fix receipt store configuration in scabbard. Previously, this was setting the
  wrong value, resulting in the receipt-store configuration to be the default
  values.

* Consolidate scabbard migrations, by including transact and sawtooth migration
  execution under the scabbard migration functions.  This removes the need for
  library consumers to use either of those libraries and prevents situations
  where the database might be only partially migrated for scabbard's purposes.

## Changes in Splinter 0.5.21

### Highlights

* Challenge authorization has been stabilized and set to default. The splinter
  daemon now requires that keys are configured that can be used for challenge
  authorization. Use `splinter keygen --system` to generate these keys. If you
  still wish to create circuits using trust authorization, when creating the
  proposal add `--auth-type trust`.

* Scabbard feature `commit-store` has been stabilized and has been removed.

### libsplinter

* Add a trait for implementing Authorization types. The trait provides methods
  for getting all the associated handlers for the defined authorization type.

* The `AuthorizationDispatchBuilder` has been updated to use these authorization
  implementations to add the related message handler. This change simplifies the
  guards around arguments that were only used if 'challenge-authorization' was
  enabled. Also simplifies the configurations required around handler tests.

* Stabilize `challenge-authorization`.

* Update unreferenced peers to properly switch connections if duplicate.
  Previously, if there was an unreferenced peer connection when one previously
  existed, it would always be replaced. This is a bad pattern if one side thinks
  the connection is a full peer and might remove the incorrect side. This commit
  updated the logic so unreferenced peers follow the same logic as full peers
  when removing a duplicate connection.

* Replace `ServiceArgValidationError` with `InvalidArgumentError`. This is
  behind the experimental feature 'service-arg-validation'.

* Update `node_id` file store to check that the node ID file doesn't only
  contain whitespace before storing the value.

* Fix the local YAML registry such that it can handle an empty file, as well as
  an empty YAML array.

### splinterd

* Stabilize `challenge-authorization` by removing the feature.

* Create a node_id module to hold `node_id` access and selection logic.

* Use the `get_node_id` method in the daemon to simplify `node_id` retrieval.

* Refactor the daemon by splitting out structs and associated implementations
  into multiple submodules under the same daemon module.

* Add authorization support to the testing framework.

* Update sawtooth dependency version to 0.6.7.

### splinter CLI

* Stabilize `challenge-authorization` by removing the feature.

* Move postgres migration behavior to its own module to match the sqlite module.

* Add the required migrations for the scabbard service state and commit hash
  store to the `splinter database migrate` subcommand.

* Add tests for the node_id store.

* Add the libscabbard sources to the CLI docker file to support the addition of
  the scabbard migrations to the CLI.

* Update sawtooth dependency version to 0.6.7.

### libscabbard

* Add a tag that specifies a circuit and service in the committed batches metric
  to fix a bug where committed batches from multiple scabbard circuits would
  override one another.

* Stabilize `commit-store` by removing the feature

* Stabilize `postgres` and `sqlite` features.

* Update sawtooth dependency version to 0.6.7.

* Rename the `diesel-receipt-store` feature to `receipt-store`.

* Update the diesel backed receipt store in scabbard to include the added
  `service_id` argument.

### gameroom

* Add missing `splinter keygen --system` for gameroom UI test.

## Changes in Splinter 0.5.20

### Highlights

* Splinterd now correctly loads the TOML configuration file from
  `SPLINTER_HOME`. Previously, splinterd would not take into consideration the
  value of `SPLINTER_HOME` or `SPLINTER_CONFIG_DIR` with respect to the
  configuration file. This change makes it so that splinterd will check both of
  those values before loading the configuration.

### libsplinter

* Stabilize `challenge-authorization` API by removing guard. This includes
  adding public keys to the circuit state objects and other API changes required
  to support challenge authorization. Note this stabilization does not include
  enabling challenge authorization at the authorization level, that will be
  stabilized separately.

### libscabbard

* Add generic `TransactCommitHashStore` which uses any
  `transact::database::Database` implementation with the configured index
  tables.  This is currently available via the experimental `commit-store` and
  stable `lmdb` features.

* Add Clone to `CommitHashStore` via a `clone_boxed` trait method and
  implementing `Clone` on `Box<dyn CommitHashStore>`

* Add `Sync` and `Send` constraints to the `CommitHashStore` trait. This brings
  the store definition in-line with all other splinter stores.

### splinterd

* Fix splinterd so it correctly loads the TOML configuration file from
  `SPLINTER_HOME`.

### splinter CLI

* Fix `splinter database migrate` so that an error isn't returned if the
  splinter_state.db file does not exist yet, as it will be created later on by
  sqlite when migrating. This fixes a regression that was introduced when a
  looping error message was removed when a file had insufficient file
  permissions.

## Changes in Splinter 0.5.19

### Highlights

* The experimental `metrics` feature has been renamed to `tap` and stabilized.

* The use of PeerAuthorizationToken as the Peer ID has been replaced by
  `PeerTokenPair` which is made up of both the other nodes authorization type
  and the local authorization type used for the local node.

* Simple back-pressure has been added to the scabbard `/batches` route. If
  there are more than 30 pending batches, the scabbard route will start
  rejecting the batch and returning TOO_MANY_REQUESTS, until the batch queue
  reduces by half.

### libsplinter

* Update `metrics` feature to use v0.17 of metrics in libsplinter. Due to
  change in v0.17 the `metrics` feature was renamed to `tap` to avoid
  conflicts.

* Make `InfluxRecorder::new` private, `InfluxRecorder::init` should be used
  instead.  The `InfluxRecorder` is available behind the `tap` feature.

* Remove clones from `InfluxRecorder`. This change removes clones from the
  influx recorder, reducing the clones of entire strings. While small, metrics
  will happen frequently as more metrics are added, so its performance impact
  should be as minimal as possible.  The `InfluxRecorder` is available behind
  the `tap` feature.

* Add connection_id to `ConnectionManagerNotification::Disconnected` and
  `ConnectionManagerNotification::NonFatalConnectionError`. This will help with
  switching peers to be uniquely identified by the identity and the local
  identity used.

* Add picked public key to `AuthChallengeSubmitResponse`. This will allow the
  node to know which public key is being used as their identity on the other
  node in the case multiple submit requests were used.

* Add local authorization type to` InboundConnection` notification. Because
  peer connections need to be identified by both the peer's authorization as
  well as how the local node was authorized, inbound connections need to return
  this information in the notification.

* Update the `InprocAuthorizer` constructor to require the node ID so it can be
  added as the local authorization.

* Add local identity to `ConnectionManagerNotification::Connected`
  notification. Because peer connections need to be identified by both the
  peer's authorization as well as how the local node was authorized, outbound
  connections need to return this information in the notification.

* Add `PeerTokenPair` for unique peer identification. Since with
  `challenge-authorization` a peer can have several connections based on the
  combinations of different local and remote authorization types, a peer must
  be uniquely identified by both the authorization type of the peer and how the
  local peer identified.

* Replace `PeerAuthorizationToken` with `PeerTokenPair` which contains both the
  peer's authorization along with the local nodes authorization type. This is
  required because a node should be able to connect to another node over
  different connections using different public keys.

* Update the `PeerManager`, `PeerMap`, `PeerConnector` and `PeerInterconnect`
  to use `PeerTokenPair` as the Peer ID for the peers.

* Update Dispatcher `PeerID` to use `PeerTokenPair` along with all of the
  message handlers.

* Update Admin service to use `PeerTokenPai`r. This required an update to the
  admin service hack so the admin service id has been updated to include both
  the peers public key and the local nodes id
  "admin::public_key::PUBLIC_KEY::public_key::PUBLIC_KEY"

* Replace panics in Authorization handlers to return an `InternalError` instead
  of panic.

* Add InternalError to `DispatchError`. This required removing `PartialEq` from
  the error.

* Move creation of accepted authorization list to the
  `AuthProtocolRequestHandlerBuilder`. Before this was done in the handler
  itself.

* Rename Local/Remote state machine structs to Initiating/Accepting. This will
   hopefully make it more clear which state is for which node in the
   authorization processes.

* Add experimental commit-store feature for database backed commit hash stores.

### splinterd

* Update `metrics-*` arguments to `influx-*`. The arguments in splinterd are
  now:

```
--influx-db <db_name> The name of the InfluxDB database for metrics collection

--influx-password <password> The password used for authorization with the
InfluxDB

--influx-url <url> The URL to connect the InfluxDB database for metrics
collection

--influx-username <username> The username used for authorization with the
InfluxDB

```

* Stabilize `tap` features. This enables sending metrics to an InfluxDB
  instance.

### libscabbard

* Remove `"factory-builder"` feature.  This feature was unnecessary and is
  required to allow for different storage configurations.

* Remove `"ScabbardFactory::new"`. The `ScabbardFactory` must now be
  constructed via the `ScabbardFactoryBuilder`.

* Stabilize `back-pressure` feature by removing it.

* Stabilize `metrics` in scabbard.

### splinter CLI

* Fix looping database error message for insufficient file permissions to
  sqlite databases.

* Breakout list of endpoints in `splinter circuit show`. The endpoints will now
  be listed separately:
  ```
      Endpoints:
          tcp://127.0.0.1:18044
          tcp://127.0.0.1:18044
  ```

## Changes in Splinter 0.5.18

### Highlights

* The `splinter admin keygen` command has been removed from the splinter CLI.
  This command duplicated the functionality of `splinter keygen` with no
  additional value.

### libsplinter

* Update connection manager to allow multiple connections for an endpoint. With
  challenge authorization, it is possible to have more then one connection
  connected to the same endpoint if the peer authorization token is different.

* Add `connection_id` to FatalConnectionError. Endpoints are no longer 1:1 with
  a connection, as such if there is a FatalConnectionError the `connection_id`
  should be returned as well.

* Update PeerManager to support multiple peers having the same endpoint. Before
  challenge authorization was implemented, endpoints to peer ID were expected to
  be 1:1. Now that is no longer true because the same peer may support multiple
  peer IDs with different authorization types that have the same endpoints.

* Add tag support for experimental metrics collection.

### scabbard

* Add the new diesel backed receipt store to scabbard as an alternative to the
  existing LMDB backed receipt store. The new receipt store is behind the
  experimental feature `diesel-receipt-store`.

* Add service ID to the batch queue metric, in order to differentiate between
  multiple scabbard instances on a node.

### scabbard CLI

* Relaxed the URL check to all for `https` endpoints in all subcommands that use
  `--url`.

### splinter CLI

* Update the `splinter database migrate` command to run migrations for the new
  diesel backed receipt store if the experimental feature
  'scabbard-receipt-store' is enabled.

* Remove the `splinter admin keygen` subcommand. This command was a duplicate of
  the `splinter keygen` subcommand.

## Changes in Splinter 0.5.17

### libsplinter

* Stabilize `node-id-store` by moving the feature to stable.

### build

* Update docker files to run the system keygen command. If enabled,
  challenge-authorization requires that a key be configured, the system keygen
  command generates this key if one does not exist.

* Add the splinter upgrade command to docker files that use the splinter
  database migrate command. Not running `splinter upgrade` before starting
  splinterd can block the daemon from starting.

* Add just recipe for linting Dockerfiles with hadolint.

## Changes in Splinter 0.5.16

### Highlights

* Stabilize v1 Trust authorization. This version fixes a race condition that was
  present in v0 Trust Authorization. Note: v0 is still supported if
  ConnectRequest is received first instead of AuthProtocolRequest message.

### libsplinter

* Fix several feature guards for `challenge-authorization`.

* Update rust documentation for NodeIdStore.

* Stabilize the `trust-authorization` feature.

### splinterd

* Stabilize the `trust-authorization` feature by moving it to default.

## Changes in Splinter 0.5.15

### Highlights

* If `challenge-authorization` is enabled the the default authorization type is
  now set to challenge and the Splinter daemon must configure at least one
  public/private key pair that can be used for identification. To configure the
  key pair run `splinter keygen --system`.

### libsplinter

* Update default authorization type to be challenge if the experimental
  features `challenge-authorization` is enabled. Otherwise, the default remains
  trust.

* Stabilize `biome-replace-keys` by removing the feature.

* Convert `network::auth::v1_handlers` into a module and break out `trust` and
  `challenge` functionality into submodules.

### splinterd

* Update challenge authorization key configuration to require at least one key
  pair if the experimental feature `challenge-authorization` is enabled.

* Add a new toml configuration option `allow_keys_file` that allows a user to
  specify the exact location of the key file. This is behind the feature
  `config-allow-keys`.

### splinter CLI

* Update `splinter circuit propose --compat 0.4` to set the authorization type
  to trust.

* Add bash completion to auto-complete subcommands and options.

### gameroom

* Update circuit creation to explicitly set authorization type to trust. This
  will keep gameroom backwards compatible if `challenge-authorization` is
  enabled. 

### Build

* Disable websocket test that hit `echo.websocket.org`


## Changes in Splinter 0.5.14

### Highlights

* Splinterd will not start if there are database migrations that need to be run
  and will give information about how to upgrade the database.

* User configurable logging with stdout, stderr, file, and rotating file log
  targets as well as fine grain control over where each log statement goes.

### libsplinter

* Replace the use of a Vec<u8> to represent a public key in the
  PeerAuthorizationToken, ConnectionAuthorizationType, Identity, and in the
  AdminService.

* Add `any_pending_migrations` to postgres and sqlite migration modules

* Fix identity comparison in PeerManager to decide between inbound/outboud
  connections. This comparison must now be between the connection's identity
  and the required local authorization used when requesting the connection
  originally.

* Stabilize `admin-service-count`. This makes `count_circuits` and
  `count_proposals`available on the `AdminServiceStore`.

* Stabilize `oauth-user-list` feature by removing it, its functionality is
  included under the `oauth` feature.

### splinterd

* Detect pending migrations and abort if migrations are required

* Add experimental `--peering-key` option to the splinterd to set which key
  will be used during challenge authorization with the peers provided with
  `--peer`

* Update `--peer` to default to using challenge authorization to connect to the
  specified unidentified peer. The `--peer` option also has been updated to
  take the format tcp+trust://ipaddr:port/ if the authorization used must be
  trust. The feature is experimental.

 * Add file base logging configuration options based on the log4j/rs pattern.
   `splinterd.toml` has new sections for loggers and appenders. Each logger can
   direct logs to one or more appenders. Each appender can be shared between
   loggers. There are two special cases, the `stdout` appender and the
   `root` logger both of which are auto added if not otherwise specified in
   the log file. Both can be overridden from `splinterd.toml` by including a
   logger or appender named `stdout` or `root` respectively. The feature is
   experimental.

*  Add new PUT method at /biome/keys to replace existing user keys

### splinter CLI

* Stabilize `user-list` feature by renaming it to `user` and adding it to
  `default`.

## Changes in Splinter 0.5.13

### Highlights

* The initial experimental  implementation of Challenge Authorization has been
  added. See [Challenge
  Authorization](https://www.splinter.dev/community/planning/challenge_authorization.html)
  for more information.

### libsplinter

* Implement Challenge Authorization. Challenge Authorization is behind an
  experimental feature `challenge-authorization`.

* Update the Admin Service to support Challenge Authorization. This is behind
  the experimental feature `challenge-authorization`.

* Update PeerManager and ConnectionManger to pass the expected authorization of
  the peer, as well as the authorization the local node must use for
  authorization. This is required to properly connect to a peer based on the
  requirements of a circuit.  This is behind the experimental feature
  `challenge-authorization`.

* Add experimental NodeIdStore with diesel implementation.  This store adds an
  alternative place to store the node id, which is saved on the first run of
  splinter.

### splinterd

* Update the creation of the AuthorizationManager to support Challenge
  Authorization.

## Changes in Splinter 0.5.12

### Highlights

* The "authorization", "authorization-handler-allow-keys" and
  "authorization-handler-rbac" features have been moved to default. This ensures
  that authorization is included in default builds of splinterd.

### libsplinter

* Update the authorization state machine for trust v0 and trust v1 to more
  closely match the design in [Challenge Authorization](https://www.splinter.dev/community/planning/challenge_authorization.html)

* Add a node id store trait for getting and setting the instances node_id, this
  trait is behind the experimental feature "node-id-store"

### splinterd

* Add integration tests around circuit creation while stopping and restarting
  nodes throughout the process

* Move stable authorization features to default to ensure default builds of
  splinterd will include authorization

## Changes in Splinter 0.5.11

### splinter CLI

*  Fix CLI signing path for `--key`. `current_user_search_path()` now searches
  `root/.splinter/keys` as expected instead of `root/.cylinder/keys`

## Changes in Splinter 0.5.10

### Highlights

* splinterd YAML circuit state is no longer supported, and the splinter CLI
  subcommand `upgrade` has been stabilized to help users port their legacy YAML
  circuit state to a database

* Both the "biome-profile" and "oauth-profile" features have been stabilized.
  This allows for the storage of basic profile information for either
  authentication scheme.

### libsplinter

* Stabilize the feature “oauth-profile” by removing it. This feature allows for
  user profile information to be collected from OAuth providers. This
  functionality is available when the “oauth” feature is enabled.

* Add user subject to oauth-profile log error message

* Wrap signing context in `Arc<Mutex>` so it can be shared with scabbard

### splinter CLI

* Stabilize the feature “upgrade”. This feature allows a user to upgrade from
  0.4 to 0.6 by upgrading the circuit YAML store to a database store.

* Fix typos in upgrade feature documentation.

### splinterd

* Stabilize the feature “biome-profile”. This feature allows for user profile
  information to be saved to the user profile store.

* Stabilize the feature “deprecate-yaml” by removing it. This feature
  deprecates the circuit YAML store, directing the user to upgrade using
  `splinter upgrade`.

* Remove references to `--storage` from configuration and tests

* Update `--state-dir` help to not reference YAML storage

* Correct the proposals location for YAML store detection

### scabbard

* Wrap signing context in `Arc<Mutex>` so it can be shared with splinterd

### Packaging

* Change splinterd `/var/lib/splinter` file ownership to `splinterd:splinterd`

* Change splinterd `/var/lib/splinter` file permissions to disallow global read

* Create `/etc/splinter/allow_keys` with splinterd postinst script

### Build

* Add a justfile recipe to build Docker images

* Add Docker build test to GitHub Actions workflows

## Changes in Splinter 0.5.9

### Highlights

* The splinter CLI subcommands `splinter authid`, `splinter role`, `splinter
  permissions`, and `splinter remove-proposal` have been stabilized.

### libsplinter

* Add native protocol structs for `NetworkMessages`. This reduces the need to
  use protobuf structs directly.

* Updated sawtooth-sabre to fix a memory leak.

### splinter CLI

* Stabilize the feature "permissions" by removing it.  This makes the subcommand
  `splinter permission list` available in the default stable builds.

* Stabilize the feature "authorization-handler-rbac".  This makes the Role-based
  Access Control subcommands `splinter role` and `splinter authid` available in
  stable builds.

* Stabilize the features "proposal-removal". This makes the subcommand `splinter
  remove-proposal` available in the default builds.

### splinterd

* Update example config files with new configuration options and documentation.

## Changes in Splinter 0.5.8

### Highlights

* It is now possible to abandon and purge circuits and remove proposals that are
  between a 0.5 node and a 0.4 node. This is allowed because the operations do
  not require confirmation from the 0.4 node. The 0.4 node will print an error
  and ignore the notification sent when the circuit is abandoned or a proposal
  is removed.

### libsplinter

* Update `AuthChallengeSubmitRequest` to have multiple `SubmitRequests` that
  have a signature and public key. This is required because a splinterd can
  support multiple key pairs and it is not clear which one is the expected
  public key without a proposal.

* Update circuit abandon, circuit purge and proposal removal to work for
  circuits and proposals that were created between a 0.5 node and a 0.4 node.

* Replace string peer id with `PeerAuthorizationToken`. This enables routing
  over the correct identity based on how the connection was authorized. The
  layer that knows what authorization type is required is the admin service, as
  such much of the public API of the `PeerManager`, `PeerManagerConnector`,
  `PeerInterconnect`, and network dispatchers had to be updated to use
  `PeerAuthorizationToken` instead of a string.

* Fix an issue with default template paths when loading circuit template. In the
  case where no paths were provided, the defaults would not be added to the
  final set of search paths. This has been corrected to include the default
  paths, both from configuration and constant values.

### splinter CLI

* Adds a `splinter user list` subcommand to the CLI, which should return all
  users registered with Splinter, through Biome or OAuth. This command currently
  returns all Splinter users, regardless of how the user registered.

## Changes in Splinter 0.5.7

### Highlights

* A new version of Trust Authorization has been added. The v0 had a race
  condition that has now been fixed in the new version. It is currently behind
  an experimental feature `trust-authorization`.

### libsplinter

* Implement v1 Trust Authorization. This version makes room for adding Challenge
  Authorization as well as fixes a race condition present in v0 Trust
  Authorization. If a `ConnectRequest` is the first message received v0 trust
  will be used. If `AuthProtocolRequest` is the first message v1 Trust will be
  used. This is required to stay backwards compatible with 0.4. v1 is behind an
  experimental features `trust-authorization`

* Update the `RefMap` to take a generic key. The RefMap is used to keep track of
  peers to know when the connection should be dropped. With the addition of
  challenge authorization the peer id is going to be updated to be an Enum
  instead of a String, as such the reference map needs to be updated to take
  keys that are other types.

* Add a default for YamlCircuitStatus in the `YamlAdminServiceStore`. If a
  circuit status is not in the yaml state, it defaults to Active. This is
  required for backwards compatibility with 0.4.

* Add authorization type and public keys to the `RoutingTable`. This information
  is required for properly routing a message once Challenge Authorization is
  implemented.

* Update the handling of proposed circuits to check node IDs in the list of
  proposed disband members to verify nodes that have previously peered still
  have an agreed upon protocol version.

### splinterd

* Changed logging libraries to the more flexible log4rs to enable future logging
  expansions.

* Add a feature "deprecate-yaml" that deprecates yaml state in splinterd in
  favor of using sqlite or postgresql. If splinterd is run with yaml state, it
  will abort and point the user to the "splinter upgrade" command, which imports
  the yaml state into the specified database.

### splinter CLI

* Add experimental feature `upgrade` to the splinter CLI. This command will port
  YAML state files to database state.

## Changes in Splinter 0.5.6

### Highlights

* The Splinter daemon now has its own sets of private/public keys that will be
  used to support Challenge Authorization.

### libsplinter

* Replace MigrationError with InternalError.  This has the effect of properly
  displaying the underlying error that may occur when accessing the database.

* Add REST endpoint to the OAuthResourceProvider, oauth/users behind the
  `oauth-user-list` feature, which lists OAuth users. This makes it easy for
  administrators to get users' Biome IDs so they can be used in RBAC
  authorization.

* Stabilized the `proposal-removal` feature. This enables removing a circuit
  proposal for the requesting node.

### splinter CLI

* Update `splinter keygen --system` to check the environment variables
  `SPLINTER_HOME` and `SPLINTER_CONFIG_DIR` for where to put the generated keys.
  Also updates the default key name to be `splinterd`.

### splinterd

* Add public/private keys to the Splinter daemon. If the experimental feature
  `challenge-authorization` is enabled, the daemon will check the configured
  "config" directory and load every key that is in the key directory. These keys
  will be used in challenge authorization.

## Changes in Splinter 0.5.5

### Highlights

* The `admin/circuit` route now returns a list of nodes in the member list
  instead of just the node id if the `SplinterProtocolVersion` is set to 2.
  This is a breaking change from previous versions.

* Stabilized `authorization-handler-rbac` feature.  This enables authorizing
  REST API users via a role-based access control system in stable builds.

### libsplinter

* Stabilized `authorization-handler-rbac` feature.

* Update admin REST API to return the nodes in the circuit member list. The
  nodes will include a public key if the `challenge-authorization` experimental
  feature is enabled.

* Add a method to the `OAuthUserSessionStore` to list OAuth users. This is
  available if the `oauth-user-list` experimental feature is enabled.

### splinter CLI

* Add experimental feature `challenge-authorization` to the splinter CLI

* Add an option to give node's public keys in the `circuit propose` command.
  The public keys will be used during challenge authorization. This is behind
  the experimental feature `challenge-authorization`.

* Add `challenge` as an option to the `--auth-type` option on `circuit propose`
  command. This is behind the experimental feature `challenge-authorization`.

### splinterd

* Stabilized `authorization-handler-rbac` feature.

* Add experimental feature `challenge-authorization` to splinterd.

* Add integration tests for the process of disbanding a circuit, while nodes
  are stopped throughout the process.

## Changes in Splinter 0.5.4

### Highlights

* Includes a number of minor improvements to mesh stability.

* Adds experimental support for including a public key in circuit nodes.

### libsplinter

* Add an optional public key to both ProposedNode and CircuitNode. This public
  key will be used during challenge authorization. This requires that the
  Circuit member list is updated to contain CircuitNodes instead of just the
  node id. This will allow storing the public key directly in the circuit. This
  is a breaking change against the YAML format, as such if a node is using the
  YamlAdminServiceStore it cannot support including public keys in the member
  list and thus will not be able to support challenge authorization.

* Fix a bug that was causing the gameroom example to fail because the Websocket
  event from Scabbard when a contract was submitted was too large. This was
  fixed reverting to use json::to_string instead of json::to_string_pretty

* Remove all mesh connection registrations on shutdown

* Remove lock contention when adding connections to mesh

* Change mesh control channel registration from level to edge, in order to
  process all control messages on a single event firing.

* Replace inproc connection internals with channels, greatly simplifying the
  implementation.

* Add simple circuit generation via CircuitBuilder and
  ScabbardCircuitBuilderVeil for creating and testing various configurations of
  Scabbard circuits.  Available through the experimental feature "node".


## Changes in Splinter 0.5.3

### Highlights

* Stabilized authorization-handler-allow-keys feature. This enables authorizing
  REST API users via the `allow_keys` file in the stable builds.

### libsplinter

* Add `Challenge` to `AuthorizationType` behind an experimental feature
  `challenge-authorization`. This is the first step in implementing Challenge
  Authorization. For more information see the [Challenge Authorization ](https://www.splinter.dev/community/planning/challenge_authorization.html)
  feature doc.

* Added new authorization messages to support challenge authorization and v1
  trust authorization.

* Stabilized authorization-handler-allow-keys feature

* Add `wait_for` and `wait_for_filter` methods, which allow a node to wait for a
  particular admin event type. The `wait_for` method uses the new `EventQuery`,
  which transforms itself into a filter to be consumed by `wait_for_filter`

### splinterd

* Stabilized authorization-handler-allow-keys feature

* Add bug fix to ensure splinterd does not start before network interfaces have
  IP Addresses

* Update the registry creation process to not require the "file://" prefix when
  a file name is being passed as a registry argument

* Add integration tests related to abandoning circuits

## Changes in Splinter 0.5.2

### Highlights

* Circuit deletion is now stabilized. Circuits can now be disbanded or
  abandoned and then purged. The CLI commands are now available in the
  `splinter` CLI.
  - [Circuit Disband](https://www.splinter.dev/community/planning/circuit_disband.html)
  - [Circuit Abandon](https://www.splinter.dev/community/planning/circuit_abandon.html)
  - [Circuit Purge](https://www.splinter.dev/community/planning/circuit_purge.html)

* Added the `proposal-removal` experimental feature, which enables removing
  circuit proposals. The experimental `splinter-circuit-remove-proposal` CLI
  command is available in the `splinter` CLI.

### splinter CLI

* Stabilized `splinter circuit disband` subcommand.

* Stabilized `splinter circuit abandon` subcommand.

* Stabilized `splinter circuit purge` subcommand.

* Added `--dry-run` to the experimental role-based access control subcommands.

* Added `splinter-circuit-remove-proposal` subcommand, guarded by the
  experimental `proposal-removal` feature.

### splinterd

* Stabilized `circuit-disband`, `circuit-abandon`, and `circuit-purge`.

* Added explicit check for existence of the state directory.

### libsplinter

* Added experimental admin event client trait and Actix Web Client (AWC)
  implementation. This trait provides a simplified wait to wait for admin
  service events.

* Added the `rbac_` prefix to database tables for the experimental role-based
  access control authorization handler.

* Fixed a timing bug that resulted in important messages being dropped. If a
  PeerInterconnect did not have the peer id for a connection before a message
  was received that message would be dropped. Now, the message is retried in
  the future, allowing time for the peering processes to finish.

* Fixed a timing bug where the process of adding internal connections to the
  mesh instance in the orchestrator or the service processor would block
  indefinitely, due to an internal channel not yet registered with mio::Poll
  before the connection was added.

* Log error and drop messages if the orchestrator's send queue is full. Before
  this commit, a full queue error would kill the loop, breaking the
  orchestrator.

* Increased default orchestrator capacity values to 512. Previously, these
  values were set to 8 messages. This is a very small restriction on the
  capacity of the message queues, especially during load.

* Added an experimental feature, `proposal-removal`, which provides the ability
  to remove circuit proposals.

* Added experimental support for count_circuits and count_proposals to
  AdminServiceStore.

### scabbard

* Implemented simple back pressure for pending batches. The submit route will
  return TOO_MANY_REQUESTS and reject the batch if the pending batch queue is
  longer than 30 batches. Scabbard will start accepting batches agains when the
  batch queue makes it to 15 batches pending. This feature is experimental
  behind `back-pressure`.

### Miscellaneous

* Switched to GitHub Actions from Travis CI for builds.

## Changes in Splinter 0.5.1

### Highlights

* Fixed a Splinter daemon connection bug in tcp:// and tcps:// transports which
  crashed the listener thread if remote disconnects occurred during protocol
  negotiation.

* The `scabbard` CLI man pages are now available. Please see the Splinter
  Website (https://www.splinter.dev/docs/0.5/references/cli/#scabbard-cli) for
  more information.

* Splinter signing code has been removed and replaced with
  [`cylinder`](https://crates.io/crates/cylinder). This was required for
  updating to the new version of Sabre.

* Scabbard and Gameroom have been updated to use Sabre v0.7. This will allow
  sabre to handle 0.5, 0.6 and 1 family versions, making splinter 0.4 work with
  0.5.

* The Splinter circuit state is now owned by the Admin Service. The Splinter
  daemon components, such as message dispatchers, now use an in memory
  `RoutingTable`. The AdminService will rebuild the `RoutingTable` on restart.

* Splinter state now supports SQLite and PostgreSQL backends. This update
  requires that the newly stabilized `splinter database migrate` command is run
  before starting up the Splinter daemon. For information see
  [Configuring Splinter Daemon Database](https://www.splinter.dev/docs/0.5/howto/configure_database_storage.html) and
  [Data Store Guidelines](https://www.splinter.dev/community/data_store_guidelines.html).
  The following data stores are now supported
      - circuits and proposals
      - admin events
      - biome
      - registry

* Stabilize `circuit-template` feature in libsplinter and the CLI. See
  [Using Circuit Templates](https://www.splinter.dev/docs/0.5/howto/using_circuit_templates.html)
  for more information

* Adds the crate::error module. This error module will provide common reusable
  errors for the library, reducing redundancy and inconsistency.

* Circuits now include a human readable display name.

* The ADMIN_PROTOCOL_VERSION has been increased to 2 to handle updates to the
  Circuit and Proposal API. Use protocol version 1 for backwards compatibility.
  The updates includes:
    - Allowed nodes list is now node_id in services
    - Circuits and Proposal now includes a display name, circuit status, and
      circuit version
    - Application metadata and comments are not optional in Proposals

* Circuits can now be disbanded or abandoned and then purged.
  - [Circuit Disband](https://www.splinter.dev/community/planning/circuit_disband.html)
  - [Circuit Abandon](https://www.splinter.dev/community/planning/circuit_abandon.html)
  - [Circuit Purge](https://www.splinter.dev/community/planning/circuit_purge.html)

* An experimental integration testing framework has been added to splinterd.
  This framework will enable the creation of integration tests that can be run
  with the normal `cargo test command`.

* Experimental support for two new authentication schemes: OAuth support
  (including Azure, Github, Google, and OpenID) and CylinderJWT.

* Experimental support for authorization, via a set of allowed keys and a
  role-based access control system.

### libsplinter
* Implement Vec::from for ConnectionMatrixEnvelope. Using Vec::from is cleaner
  than using the  ConnectionMatrixEnvelope::take_payload.

* Remove the remaining deprecated Network structs. Several tests still relied
  on the old Network implementation. The tests were updated to use Mesh or the
  peer module so the deprecated structs could be removed.

*  Add `PeerManagerBuilder` to make creating a `PeerManger` easier and reduce
  errors by starting the background thread on build. The builder matches the
  pattern of the `ConnectionManagerBuilder`.

* Add `subscribe_sender`  to  `PeerManagerConnecter` which takes a Sender<T>
  which can be used to get a PeerManagerNotification and adds the ability to
  convert the notification to another type. This change updates the PeerManager
  to match the ConnectionManager. Also adds `unsubscribe` to the connector
  which allows a subscriber to no longer receive notifications.

* Improve log messages around peer creation.

* Adds missing Rust API documentation to the peer module.

* Update frame negotiation to result in a handshake failure if the connection is
  terminated mid-negotiation when using TCP or TLS.

* Rewrite WsTransport in tungstenite and add SSL support. The websocket crate
  had serious limitations which manifested when attempting to implement TLS
  support. After more analysis, Tungstenite looks better overall, so
  re-implemented with that dependency instead. This is experimental behind the
  `ws-transport` feature.

* Clean up circuit template API:
  - Remove '$()' from template argument names
  - Reworks the `apply_rules` method so the organization makes it clear
    which `rule` is creating the value.
  - Remove "a:" and "r:" prefix to template arguments.
  - Changes the `CircuitTemplate` `into_builders` method to `apply_to_builder`,
    which takes in a `CreateCircuitBuilder` and applies the template rules using
    the builder .with_* methods.

* Add reader and writer traits for the routing table, as well as an in memory
  implementation. The routing table will replace the splinterd uses of
  SplinterState, most widely used in the Handlers for messages that are
  dispatched.

* Add RegistryStore with YAML, SQLite and PostgreSQL backend implementations.
  This enables being able to config different backends for the registry. Before,
  only YAML was supported.

* Add AdminServiceStore with YAML, SQLite and PostgreSQL backend
  implementations. This enables storing circuit and proposal state in SQLite and
  PostgreSQL database while staying backward compatible with the 0.4 YAML state.

* Update AdminService to use the AdminServiceStore for storing circuit and
  proposal state. This also requires updating the AdminService to use a
  RoutingTableWriter to update the RoutingTable for new circuits so that the
  splinterd message handlers can properly route messages.

*  Replace allowed_nodes list with node_id in service definitions. Only one
  node ID is currently supported.

* Replace auth with authorization_type in Circuit Update the AdminServiceStore
  Circuit struct to have authorization_type instead of auth. This makes the use
  of authorization_type consistent between Circuit and ProposedCircuit.

* Replaces the `libsplinter::signing` module with use of the `cylinder`
  crate. This change removes the `sawtooth-sdk` dependency as well as the
  `sawtooth-signing-compat` feature.

* Adds the crate::error module that provides common reusable errors for the
  library, reducing redundancy and inconsistency. The included errors are:
    - InternalError
    - ConstraintViolationError
    - InvalidStateError
    - InvalidArgumentError
    - ResourceTemporarilyUnavailableError

* Guard all admin service code with an `admin-service` feature. The admin
  service code is quite large and is not alway required. This feature is stable.

* Move all migrations file to a top level location. Running migrations from
  multiple locations can cause conflicts because there is only one table for
  keeping track of what transactions have already been run in the database. That
  conflict can be removed by having all migrations in one place and only
  running them once.

* Stabilize the "store-factory" features. This feature guarded the StoreFactory,
  which is used to dynamically create the stores for the configured database
  backend.

* Add a separate protocol version for the AdminService. Before the admin
  service used the same protocol version as the admin service REST API. However,
  the REST API and the protobuf messages have different protocols and should be
  treated as such.

* Increase the AdminService protocol level to support handling new requests
  that cannot be handled by v0.4 AdminService.

* Replace the in memory only implementation of storing AdminServiceEvents with
  storying the AdminServiceEvents in the AdminServiceStore. This fixes a bug
  where the events were only stored in memory so they could not be retrieved
  after a restart.

* Use Message::parse_from_bytes instead of protobuf::parse_from_bytes. This
  fixes a breaking change introduced by protobuf 2.19.

* Add support to the AdminService for handling a CircuitDisband request.
  Disbanded a circuit removes all networking support from the circuit while
  maintaining the circuit state. This is the "friendly" approach for removing a
  circuit that requires agreement from all members.

* Add support to the AdminService for handling a CircuitAbandon request.
  Abandoning a circuit is similar to disbanded but does not require agreement
  from the other members of a circuit. This will result in broken circuit
  functionality for the other members.

* Add support to the AdminService for handling a CircuitPurge request. After a
  circuit has been disbanded or abandoned, a  purge request will remove all
  circuit state related to the circuit, including service state.

* Add `purge` to the Service trait. `purge` will enable removing the services
  associated datafiles when a circuit is purged.

* Refactor authentication system to allow more supported authentication methods
  beyond the provided Biome credentials

* Add support for OAuth authentication

* Add support for Cylinder JWT authentication

* Add experimental permissions extensions to REST API Resource construction

* Add experimental support for authorization via an allowed keys file.  All keys
  placed in the file are allowed access to any permission-guarded REST API
  routes. This authorization method is intended namely for system
  administrators.

* Add experimental support for authorization via role-based access control.
  Users, by ID or public key, may be assigned roles which, in turn, are made up
  of a set of permissions. This allows control over user access at a
  finer-grained level than "allowed keys".

* Add experimental support for maintenance mode authorization.  This mode set
  sets the REST API in read-only mode for all role-based access control users.
  While enabled, any write permissions are denied, but read permissions continue
  to work as configured.  Maintenance mode has no effect on users authorized
  via the allowed keys file.

* Add experimental support for Splinter Metrics. This required implementing an
  InfluxDB specific implementation of the Recorder trait from the
  [metrics crate](https://crates.io/crates/metrics). This will enable us to gain
  insight into the inner working of Splinter during load and performance
  testing by adding metrics throughout the code. Several initial metrics were
  added throughout the code.

* Add experimental BiomeClient trait and an reqwest implementation.

* Add `threading::lifecycle::ShutdownHandle`. Anything which runs a thread will
  need to implement ShutdownHandle in the future, which will standardize
  libsplinter's approach to shutdown (a.k.a. joining threads).

* Implement ShutdownHandle trait for the following components (removing the old
  shutdown pattern):
    - ServiceProcessor
    - Orchestrator
    - PeerManager
    - PeerInterconnect
    - ConnectionManger
    - Mesh
    - RemoteYamlRegistry
    - DispatchLoop

* Replace `insert_node` with `add_node` and `update_node` in the registry.

* Add experimental initial Actix Web 3 support. This adds the start of pieces
  we need to easily standup and manage the REST API without any wrapper code
  (such as the approach taken with the Actix Web 1 support). This includes
  support for a RestApi struct and its lifecycle (RestApiBuilder ->
  RunnbleRestApi -> RestApi -> shutdown).

* Version 2PC and add v2 implementation. Moves the existing 2PC implementation
  to a v1 submodule and creates a new v2 implementation that is not compatible
  with v1. V2 disables custom verifiers and only allows the coordinator to
  create proposals. This fixes several bugs found when running under scabbard
  under load. The AdminService still requires v1.


### splinter CLI

*  Add man pages for all stable `scabbard` CLI commands

* Update CLI man page 'SEE ALSO' link to point the Splinter website
  https://www.splinter.dev/docs/0.5/

* Update CLI signing code to use cylinder. Replaces the use of the
  `splinter::signing` module with the cylinder crate.

* Update `splinter database migrate`  to run migrations for both SQLite and
  PostgreSQL.

* The CLI has been updated to handle the updated circuit and proposals API
  returned with ADMIN_PROTOCOL_VERSION 2.

* Add `--compat=0.4` flag to `splinter circuit propose`. This flag enforces
  that the circuit created by the proposal will be compatible with v0.4 of
  Splinter.

* Add experimental support to `splinter cert generate` to generate REST API
  certificates for https.

* Add experimental support to `splinter circuit` to disband, abandon, and
  purge circuits.

* Add experimental support for managing role-based access with the `splinter
  role` and `splinter authid` subcommands.  The `role` subcommands may display,
  create, update or delete roles. The `authid` subcommands may display, create,
  update or delete user or key authorizations.

* Fix a bug that enforced that all service arguments were wrapped in a Vec.
  This made it impossible to provide arguments that were expected to be just a
  string, such as scabbard "version"

### splinterd

* Update the daemon to use the `PeerMangerBuilder`.

* Update accept thread to log errors and continue instead of exiting on an
  AcceptError.

*  Add wss:// handling support to the daemon If TLS is enabled, then create the
  WsTransport using the same configuration as the socket-based TLS Transport.

* Update the splinterd to use the store factory to configure all stores with
  the database value provided by the config. This enables using a SQLite or
  PostgreSQL database for circuit state, registry, biome, etc.. For backwards
  compatibility `--storage` is hidden but still available and if `--storage
  yaml` is provided the AdminService will use the YAML implementation of
  AdminServiceStore.

* Add experimental HTTPS support for the splinterd REST API.

* Remove `--enable-biome` flag. Removes the `enable_biome` option for the
  Splinter daemon. The inclusion of Biome is now a compile-time choice rather
  than run-time. Previously, the `enable_biome` option was necessary to be able
  to run splinterd without a database, but soon a database will always be
  required for splinterd. For backwards compatibility, the `--enable-biome` CLI
  flag is kept but marked as hidden.

* Add experimental support for configuring metrics. If the `metrics` flag is
  enabled the following flags are available that will be used to connect to a
  running InfluxDB database:

```
--metrics-db <metrics_db>  The name of the InfluxDB database for metrics
                           collection
--metrics-password <metrics_password> The password used for authorization with
                                      the InfluxDB
--metrics-url <metrics_url>  The URL to connect the InfluxDB database for
                             metrics collection
--metrics-username <metrics_username> The username used for authorization with
                                      the InfluxDB
```

* Add support for the use of the CylinderJWT authentication method

* Add experimental use of the OAuth authentication method

* Add experimental use of the "allowed keys" authorization method

* Add experimental use of the role-based access control authorization method

* Add experimental use of the maintenance mode authorization method


### scabbard CLI

* Update CLI signing code to use cylinder. Replaces the use of the
  `splinter::signing` module with the cylinder.

* Add scabbard support for SPLINTER_REST_API_URL This change modifies
  scabbard's URL handling to match that of the splinter CLI command.

### scabbard

* Update scabbard to use cylinder. Replaces the use of the `splinter::signing`
  module with the cylinder.

* Update sabre version to 0.7.

* Update the CLI to use the ScabbardClientBuilder.

* Implement `Service.purge()` method so scabbard state can be removed when a
  circuit is purged.

* Add a version service argument that tells scabbard what version of 2PC to use.
  v2 of 2PC fixes several bugs that were found while testing scabbard under
  load. But for backwards compatibility the with v0.4 Splinter v1 2PC must
  still be supported.

* Use correct state directory for scabbard files This initializes the scabbard
  service with the same state directory used for the rest of the daemon, instead
  of using scabbard's default. This fixes a bug where the scabbard files were
  always put in the default directory.

* Update the scabbard service to handle CSV passed in as service argument.
  Before the list of admin keys and peer services had to be decoded using json.
  Now the arguments can be passed in as either JSON or CSV.

* Add a builder for the ScabbardClient.

### gameroom

* Use the gameroom circuit template in Gameroom daemon. Updates the
  `propose_gameroom` REST API endpoint function to use a CircuitCreateTemplate
  to generate the CreateCircuit admin message used to propose a gameroom.

* Update sabre version to 0.7.

* Replace the use of sawtooth-sdk with transact-sdk-javascript in the
  gameroom-app.

### build

* Dockerfiles have been updated from bionic to focal.

## Changes in Splinter 0.3.18

### Deprecations and Breaking Changes

* The default network endpoint for `splinterd` now contains a TLS protocol
  prefix, `tcps://`. When starting `splinterd` with the `--no-tls` flag, a valid
  TCP endpoint must be provided (with the `network_endpoint` configuration
    setting or `--network-endpoint`  command option). Using the default network
    endpoint with the TLS protocol prefix will cause `splinterd` to fail.

* The `splinterd`  `service-endpoint` configuration setting and
  `service_endpoint` command option are now available only with the experimental
  `service-endpoint` feature. The command option and config setting no longer
  have an effect if the `service-endpoint` feature is not enabled.

* The `PeerManager` and its related components have been moved to the
  `splinter::peer` module.

For upgrade information, see
[Upgrading to Splinter 0.3.18 from Splinter 0.3.17](https://github.com/Cargill/splinter-docs/blob/master/docs/upgrading/splinter-v0.3.18-from-v0.3.17.md).

### libsplinter

* Fix a bug where the Transact scheduler was not shut down if a transaction was
  invalid.

* Update the peer manager to handle the case where two peers connect to each
  other simultaneously.

* Fix a bug where a failure to authorize on peer reconnection would result in
  retrying authorization indefinitely. This failure now causes the connection to
  be removed.

* Update the peer manager to remove inbound connections for a peer when it is
  disconnected, then retry connecting to the peer's endpoints.

* Updated `GET /ws/admin/register/{type}` endpoint to check if the admin service
  is running before creating a websocket connection. If it is not running, the
  endpoint now returns a 503 service unavailable response.

* Add `on_reconnect` callback to `WebsocketClient` that is called before
  attempting to reconnect to a server.

* Move the peer manager from the `splinter::network::peer_manager` module to the
  `splinter::peer` module.

### splinterd

* Change the default service and network endpoints from TCP to TLS and require a
  valid TCP endpoint when using the `--no-tls` flag.

* Add the experimental `service-endpoint` feature and use it to guard the
  `--service-endpoint` CLI option and `service_endpoint` configuration setting.
  The option and setting no longer have an effect if the `service-endpoint`
  feature is not enabled.

* Remove the following features as compilation options so they are always
  compiled:
    - `config-command-line`
    - `config-default`
    - `config-env-var`
    - `config-toml`

### Splinter CLI

* Set crate dependencies to log level `Warn` to reduce noise in the logs.

### Scabbard

* Add the following to support checking a scabbard service's current state root
  hash:
    - `ScabbardState::current_state_root` method
    - `Scabbard::get_current_state_root` method
    - `GET /state_root` REST API endpoint
    - `ScabbardClient::get_current_state_root` method
    - `scabbard state root` CLI subcommand

### Gameroom

* Update gameroom application to wait for `/submit` response and display any
  error messages from the response.

* Update docker compose files to use static IP addresses for all containers.
  This fixes a bug where proxy IP addresses would become out-of-date when
  containers were restarted.

* Fix a bug that made it impossible to create new gamerooms after an instance of
  gameroomd reconnected to a previously restarted splinter node.

### Private Counter and Private XO

* Remove the Private Counter and Private XO example applications from the
  `splinter` repository. These applications are incomplete examples that were
  maintained primarily to test external services; external services are now
  considered experimental, so the applications are no longer needed.

### Miscellaneous

* Update several error and log messages to be more descriptive.


## Changes in Splinter 0.3.17

### Highlights

* Connection handling has been updated to use a dedicated connection manager,
  which provides consistent reconnection and heartbeat logic across both network
  and component connections.

* Peering is now performed at the networking layer, rather than by the admin
  service. The peer manager allows other components (such as the admin service)
  to request the creation of a peer; these requests are counted to make sure
  connections remain as long as needed. A peer interconnect component provides
  the capability for higher-level components to send messages to peers without
  directly interacting with the underlying connection layer.

* The Scabbard service, along with its client and other associated code, has
  been moved from the `splinter` crate to a new
  [`scabbard` crate](https://crates.io/crates/scabbard).

* The key registry and node registry have been merged into a single registry.
  All nodes in the registry now have a list of keys that is used by the admin
  service to verify circuit management permissions.

### Deprecations and Breaking Changes

* The scabbard service is no longer provided by the `splinter` crate. The
  scabbard service and its client now belong to the `scabbard` crate.

* The names of several `splinterd` configuration settings and CLI options have
  changed. For more information, see the [`splinterd` section](#splinterd).

* Admin services now negotiate the protocol version after authorization. This
  change breaks compatibility between the 0.3.17 release and previous releases.

* The key registry has been removed. Its functionality has been replaced by a
  new `keys` entry for nodes in the Splinter registry. Registry configurations
  will need to be updated. (See the upgrade document, linked below, for more
  details.)

* The `/admin/nodes` endpoints have been moved to `/registry/nodes`.

* The Gameroom daemon (`gameroomd`) now uses Biome for user and credential
  management. There is a new "migrate database" step to correctly populate the
  Biome tables in the database. In addition, the Splinter nodes backing
  `gameroomd` need to be run with Biome enabled and must be connected to a
  database. Gameroom's docker-compose files have been updated to handle these
  changes. (See the upgrade document, linked below, for information on starting
  Gameroom manually.)

For upgrade information, see
[Upgrading to Splinter 0.3.17 from Splinter 0.3.16](https://github.com/Cargill/splinter-docs/blob/master/docs/upgrading/splinter-v0.3.17-from-v0.3.16.md).

### libsplinter

* Remove the `matrix` feature as a compilation option. This code is now always
  compiled.

* Make the `service::rest_api` module public.

* Update circuit-definition serialization to include the node IDs.

* Make the `collections` module private to the `splinter` crate.

### Admin Service

* Update the admin service to log (but otherwise ignore) errors when restarting
  services.

* Add the `AdminKeyVerifier` trait (with an implementation for `RegistryReader`)
  for verifying proposal/vote permissions.

* Fix a bug in the service restart process that caused a deadlock in some
  scenarios.

* Add a `state_dir` argument to configure where the admin service stores files.

#### Splinter Registry

* Add the `registry` feature for optionally compiling the Splinter registry.

* Change the names of the following items:

  - Module `node_registry` is now `registry`
  - `NodeRegistryError` is now `RegistryError`
  - `NodeRegistryReader` is now `RegistryReader`
  - `NodeRegistryWriter` is now `RegistryWriter`
  - `RwNodeRegistry` is now `RwRegistry`
  - `LocalYamlNodeRegistry` is now `LocalYamlRegistry`
  - `RemoteYamlNodeRegistry` is now `RemoteYamlRegistry`
  - `UnifiedNodeRegistry` is now `UnifiedRegistry`

* Add methods for cloning an `RwRegistry` as a `Box<RegistryReader>` or
  `Box<RegistryWriter>`.

* Update `NodeIter` to be an `ExactSizeIterator`.

* Update `LocalYamlRegistry` to reload changes if its file is modified.

* Move `/admin/nodes` endpoints to `/registry/nodes` and introduce a
  `REGISTRY_PROTOCOL_VERSION` version number for `/registry` endpoints.

### splinterd

* Change the names of `splinterd` command options and configuration settings to
  be consistent.

  Configuration settings:
    - `admin_service_coordinator_timeout` is now `admin_timeout`
    - `bind` is now `rest_api_endpoint`
    - `heartbeat_interval` is now `heartbeat`
    - `registry_auto_refresh_interval` is now `registry_auto_refresh`
    - `registry_forced_refresh_interval` is now `registry_forced_refresh`

  Command options:
    - `--advertised-endpoint` is now `--advertised-endpoints`
    - `--bind` is now `--rest-api-endpoint`
    - `--network-endpoint` is now `--network-endpoints`
    - `--peer` is now `--peers`
    - `--registry` is now `--registries`

  For more information, see
  [Upgrading to Splinter 0.3.17 from Splinter 0.3.16](https://github.com/Cargill/splinter-docs/blob/master/docs/upgrading/splinter-v0.3.17-from-v0.3.16.md).

* Add the `--config-dir` CLI option and `SPLINTER_CONFIG_DIR` environment
  variable for configuring the location of a `splinterd` configuration file to
  load.

* Allow the following TLS-related configuration settings and CLI options to use
  absolute or relative paths:
    - `tls_ca_file` and `--tls-ca-file`
    - `tls_client_cert` and `--tls-client-cert`
    - `tls_client_key` and `--tls-client-key`
    - `tls_server_cert` and `--tls-server-cert`
    - `tls_server_key` and `--tls-server-key`

* Move the `biome`, `biome-key-management`, and `biome-credentials` features
  from `experimental` to `default`.

* Fix a typo in the `--enable-biome` option that was causing the flag to never
  be set to true.

* Add a context string to the `UserError::IoError` variant to provide more
  details when one of these errors is encountered.

* Make the `node_id` configuration setting and CLI option (`--node-id`)
  optional, and provide a randomly generated default. An automatically generated
  node ID is an ‘n’ followed by a random five-digit number, such as `n12345`.

* Add a `SPLINTER_HOME` environment variable. When set, other directory
  configuration settings (or command options) do not need to be used to change
  the location of the TLS certificate directory, state directory, and config
  directory. This variable sets the following items:
    - `tls_cert_dir` is set to `$SPLINTER_HOME/certs`
    - `state_dir` is set to `$SPLINTER_HOME/data`
    - `config_dir` is set to `$SPLINTER_HOME/etc`

* Add a new `--state-dir` option to the `splinterd` CLI. This option configures
  the directory location of registry files if the `--storage` configuration
  option is set to `yaml`. The `--state-dir` CLI option overrides the default
  value (`/var/lib/splinter/`) and any value set by the `SPLINTER_STATE_DIR`
  environment variable.

### `splinter` CLI

* Add the `splinter registry build` command for constructing Splinter registry
  files. This command queries a `splinterd` `/status` endpoint for node
  information, then combines this information with user-specified public key
  files to add the node to a registry file.

* Move the  `database` and `database-migrate-biome` features from `experimental`
  to `default`.

### Scabbard

* Move the `client` feature (formerly the `scabbard-client` feature in
  `libsplinter`) from `experimental` to `stable`.

* Remove the `scabbard-get-state` feature as a compilation option. This
  functionality is now always enabled.

* Rename the scabbard client's `Error` struct to `ScabbardClientError`.

* Update the `ScabbardClient::submit` method to take `Duration` rather than a
  `u64` of seconds.

* Update the `ScabbardState::get_state_with_prefix` method to return an empty
  iterator when the prefix is not in state.

### Documentation

* Add man pages for the `splinter health` command and its subcommands.

* Make general improvements and corrections to the man pages and Rust API
  documentation.

### Gameroom

* Update the database creation and migration process to prevent migrations from
  running when not necessary.

* Run database migrations for gameroom tests to verify that the migrations are
  correct.

* Replace the key registry and static node registry file with a dynamically
  constructed registry file.

* Player names now appear in the UI as truncated public keys rather than
  user-friendly names. This is a side effect of removing the key registry; it
  will be addressed in a future release.

* Update both gameroom databases to preserve state across restarts with Docker
  volumes.

* Update `gameroomd` to use Biome for user registration and login.

### Private Counter and Private XO

* Remove registry files because they are no longer required by `splinterd` and
  are not needed for these example applications.

### Miscellaneous

* Update the Splinter justfile to run lint checks on all Splinter crates and
  exit on warnings.

* Add the Splinter logo to the Splinter repository's README.


## Changes in Splinter 0.3.16

### Highlights

* The Splinter daemon, `splinterd`, can be configured with multiple read-only
  node registry files (in YAML format). Specify registries with file paths
  (prefixed with `file://`) or HTTP(S) URLs (prefixed with `http://` or
  `https://`).

### Deprecations and Breaking Changes

* The `--transport` option has been removed from the `splinterd` command. The
  `splinterd` `transport` configuration setting was also removed.

* The protocol prefix for TLS transport has been changed to `tcps://`. The old
  prefix, `tls://`, is still supported but is considered deprecated.

* The TLS options and configuration settings for `splinterd` are now prefixed
  with `tls`.

  Changed configuration settings:

    ```
    cert_dir  -> tls_cert_dir
    ca_cert -> tls_ca_file
    client_cert -> tls_client_cert
    client_key -> tls_client_key
    server_cert -> tls_server_cert
    server_key -> tls_server_key
    insecure -> tls_insecure
    ```

  Changed `splinterd` command options:

    ```
    --cert-dir  -> --tls-cert-dir
    --ca-file -> --tls-ca-file
    --client-cert -> --tls-client-cert
    --client-key -> --tls-client-key
    --server-cert -> --tls-server-cert
    --server-key -> --tls-server-key
    --insecure -> --tls-insecure
    ```

* A required `version` field has been added to all config objects. TOML
  configuration files should have a `version = 1` added at the beginning of the
  file.

* The `splinterd` configuration settings `registry_backend` and `registry_file`
  are no longer available. The related `splinterd` command options
  `--registry-backend` and `--registry-file` are also gone. Instead, use the
  `registries` configuration file setting or the `--registry` option with the
  `splinterd` command.

* Nodes may now have multiple network endpoints. The `splinterd` configuration
  setting `network_endpoint`, which was previously a single value, has been
  changed to `network_endpoints` and now takes an array of values. The
  `splinterd` command line option `--network-endpoint` remains the same, but can
  be specified multiple times. This change also affects node registry files,
  circuit proposals, and REST API responses; see the upgrade document below for
  more details.

For upgrade information, see
[Upgrading to Splinter 0.3.16 from Splinter 0.3.15](https://github.com/Cargill/splinter-docs/blob/master/docs/upgrading/splinter-v0.3.16-from-v0.3.15.md).

### libsplinter

* Change the following features from experimental to stable. They can be enabled
  with the “stable” feature flag (instead of the “experimental” feature flag),
  in addition to being enabled individually. These features will be used by the
  default and stable Docker images published at
  [splintercommunity](https://hub.docker.com/u/splintercommunity).

    - `biome-key-management`
    - `biome-credentials`
    - `registry-remote`
    - `rest-api-cors`

* Remove the following features (no longer available as compilation options),
because the functionality is now available by default:

    - `node-registry-unified`

* Update the `Dispatcher` code:

    - Add `From` implementation for `ProtoConversionError` in the
    `dispatch_proto` module.  This will allow for uses of the various proto
    conversion traits to be used with the `?` operator in handlers.
    - Add a function to the `Handler` trait for `match_type`, which more tightly
    couples the message type to the handler.  This change greatly reduces the
    possibility that the handler and the type it is registered to could get out
    of sync.
    - Add a trait for message sending that can be coupled to the Source via
    generics.  A new module, `dispatch_peer`, implements this new trait
    specifically on `PeerId` for `NetworkMessageSender`, which supports the
    existing usages.

* Switch the dispatch `Handler` trait from using generics to using associated
  types.

* Add a new generic parameter on dispatcher-related structs, `Source`, that can
  be either `ConnectionId` or `PeerId`.  The generic type defaults to `PeerId`
  for backwards compatibility.

* Add a step to check if a request has method `OPTIONS` if `CORS` is enabled.

* Allow adding a NetworkMessageSender after a Dispatcher has been created.

* Add an optional whitelist to the RestApi object used to instantiate CORS
  middleware.

* Improve the `InprocTransport` `ConnectionRefused` error when there's no
  `Listener`.

* Improve the error message when a service cannot be initialized.

* Add the method `port_numbers` to `RestApiShutdownHandle`, which returns a list
  of ports that the REST API was bound to when it was created.

* Simplify the connection manager error implementation.

* Improve the node registry:

  - Remove the no-op node registry since it is no longer used.
  - Update node registry errors to match the standard pattern used elsewhere in
    Splinter.
  - Rename `YamlNodeRegistry` to `LocalYamlNodeRegistry` to prevent confusion
    with the `RemoteYamlNodeRegistry`.
  - Add the `RemoteYamlNodeRegistry` for reading node registry files over
    HTTP(S).
  - Improve node registry documentation.
  - Replace the `Node` constructor with the `NodeBuilder`
  - Add a newline to the end of local YAML node registry files.

* Switch two phase coordinator timeout from milliseconds to seconds.

#### Testing

* Re-enable two mesh tests

#### Biome

* Remove the use of concrete stores from the Biome REST API. This makes it
  possible to test `BiomeRestResourceManager`.

* Add a `CredentialsStore`, `RefreshTokenStore`, `UserStore`, and `KeyStore`
  implementation that uses `Arc<Mutex<HashMap>>>` to store objects in memory.

* Change the source field for `<store_error>::StorageError` to
  `Option<Box<dyn Error>>`. This makes it easier to handle cases where the
  underlying error isn't interesting or important to surface, or if the error
  doesn't implement the `Error` trait.

* Fix a bug in the route handler for `GET /biome/key/{public_key}`,
  `PATCH /biome/key/{public_key}` and `DELETE /biome/key/{public_key}`.

* Fix a performance issue when updating a user's password.

* Add initial unit tests for the Biome REST API.


### splinterd

* Update the response for splinterd's `GET /status` endpoint to include the
  node's list of network endpoints.

* Add the `advertised_endpoints` configuration setting and
  `--advertised-endpoints` command option to `splinterd`. This value is used to
  define the node's publicly accessible network endpoints if they differ from
  the node's bound network endpoints. The `advertised_endpoints` setting is
  exposed via `GET /status` REST API endpoint.

* Add the `display_name` configuration setting to `splinterd`. This value is
  used to give the node a human-readable name. The `display_name` setting is
  exposed via splinterd's  `GET /status` REST API endpoint.

* Update the `splinterd` `main` method to use the `Path` struct for building
  file paths.

* Remove the `--transport` option from the `splinterd` command.

* Add a required `version` field to all config objects and check that each
  config object added to the final `ConfigBuilder` object has the correct
  version

* Add support for './' and '../' in file paths for files necessary for
  `splinterd` configuration.

* Remove `registry_backend` and `registry_file` config options

* Add the configuration options `registries`, `registry_auto_refresh_interval`,
  and `registry_forced_refresh_interval`.

* Refactor the `splinterd` local registry location configuration.

### CLI

* Remove the `keygen` feature (no longer available as compilation option); the
  `splinter keygen` subcommand is now available by default.

* Add the long option `--url` to `splinter health status`.

### Documentation

* Add man pages for the following commands: `splinter`, `splinterd`,
  `splinter cert`, `splinter database`, and `splinter keygen`.

  To display a man page with the `man` command, use the dashed form of the name,
  where each space is replaced by a dash. For example, `man splinter-cert`.

### Gameroom

* Fix a non-deterministic failure in a gameroom integration test.

* Update code to support change to multiple endpoints for each node

* Add a database migration for the change from `endpoint` to `endpoints` for the
  `gameroom_member` table.

### Miscellaneous

* Add a justfile to support using `just` for simple cross-repo building,
  linting, and testing.
* Log a commit hash in the `splinter-dev` image.
* Add additional cleanup to the `splinter-dev` image.
* Update the `protoc-rust` dependency to version 2.14.


## Changes in Splinter 0.3.15

### Highlights

* Command name change: Use `splinter circuit propose` to propose a new circuit
  (instead of  `splinter circuit create`).
* The `splinter node alias` subcommands have been removed.
* Default values for the circuit management type and service type are now
  configurable with the environment variables `SPLINTER_CIRCUIT_MANAGEMENT_TYPE`
  and `SPLINTER_CIRCUIT_SERVICE_TYPE`.
* Biome routes for user keys no longer require a user ID.
* The splinterd REST API endpoints for reading and proposing circuits are now
  in the default compilation target. Previously, this functionality required
  the "experimental" feature flag during compilation.
* The "biome" and "postgres" features are now available with the "stable"
  feature flag (instead of "experimental") and will be included in the default
  and stable Docker images published at
  [splintercommunity](https://hub.docker.com/u/splintercommunity).
* There is a new [splinter-ui](https://github.com/Cargill/splinter-ui)
  repository for Canopy and saplings.

### Deprecations and Breaking Changes

* Biome routes for user keys no longer require a user ID.
    - The endpoint `biome/users/{user_id}/keys` is now `biome/keys`
    - The endpoint `biome/user/{user_id}/keys/{public_key}` is now
      `biome/keys/{public_key}`
* The `splinter circuit create` subcommand is now `splinter circuit propose`.
* The `splinter node alias` subcommands have been removed. This functionality
  will be replaced by `splinter circuit template` subcommands in an upcoming
  release.

For upgrade information, see [Upgrading to Splinter 0.3.15 from Splinter 0.3.14](https://github.com/Cargill/splinter-docs/blob/master/docs/upgrading/splinter-v0.3.15-from-v0.3.14.md).

### libsplinter

* Move the "biome" and "postgres" features from experimental to stable. They
  can be enabled with the “stable” feature flag (instead of the “experimental”
  feature flag), in addition to being enabled individually. These features
  will be used by the default and stable Docker images published at
  [splintercommunity](https://hub.docker.com/u/splintercommunity).
* Remove the following features (no longer available as compilation options),
  because the functionality is now available by default:
    - `biome-rest-api`
    - `circuit-read`
    - `proposal-read`
* Remove the following features (no longer available as compilation options):
    - `database` - Redundant; its functionality can be accessed with the
       "postgres" feature,
    - `json-web-tokens` - Deemed unnecessary, because using authorization tokens
       throughout the Biome REST API is always required,
* Clean up and expand the transport module:
    - Remove unused errors, PollError and StatusError, from the transport
      module.
    - Remove transport status enum.
    - Implement `Display` for transport errors.
* Refactor the database module:
    - Refactor the constructor for `ConnectionPool` to `ConnectionPool::new_pg`
      to support multiple backends.
    - Replace `database::DatabaseError` with `database::ConnectionError`
      because the only valid database errors were connection related.

#### Testing

* The REST API tests now shut down after the tests finish.

#### Biome

* Change two Biome routes so that the user ID is not required in the route. The
  user ID is now derived from the provided access token.
    - `biome/users/{USER-ID}/keys` has changed to to `biome/keys`
    - `biome/users/{USER-ID}/keys/{public_key}` has changed to
      `biome/keys/{public_key}`
* Add a "refresh_token"  feature to the list of experimental features. When
  this feature is enabled, `biome/login` now returns a refresh token. (Refresh
  tokens are sent to the `POST /biome/token` endpoint to generate new access
  tokens without having to collect credentials when the short-lived access
  tokens expire.) Also, Biome has a new endpoint, called `/biome/tokens`, that
  validates refresh tokens and returns a new access token to the API consumer.
* Add a `/biome/logout` route.
* Rename and update several Biome items:
    - `SplinterUserStore` is now `DieselUserStore`
    - `SplinterCredentialsStore` is now `DieselCredentialsStore`
    - `SplinterUser` is now `User`.
    - `UserCredentials` is now `Credentials`.
    - `CredentialsStore` trait method `get_usernames` is now `list_usernames`.
    - Remove the generic type definition from the `UserStore` trait and the
      `CredentialsStore` trait.
    - Add the `update_keys_and_password` method to `KeyStore` trait.
* Add the `new_key_pairs` field to the `PUT /biome/users/{user_id}` payload.
     ```
     {
       "username":"test@test1.com",
       "hashed_password":"Admin2193!",
       "new_password":"hello",
       "new_key_pairs":[{
           "display_name":"test",
           "encrypted_private_key":"<encryped_private_key>",
           "public_key":"<private_key>"
       }],
    }
    ```
* `BiomeRestResourceManager` now requires `CredentialsStore` to be created.

#### Protobuf

* Add `FromProto`, `FromNative`, `IntoBytes`, and `FromBytes` to the protobuf
  module.
* Add a `ViaProtocol` generic parameter to the `FromBytes` and `ToBytes` traits.
  This allows for auto-implementations of the types for implementers of
  FromNative and FromProto.
* Add a `splinter::protos::prelude` module.

### CLI

* Change the following features from "experimental" to "stable". They can now
  be enabled with the “stable” feature flag, in addition to being enabled
  directly, and will be used by the default and stable Docker images published
  at [splintercommunity](https://hub.docker.com/u/splintercommunity).
    - `database-migrate-biome`
    - `circuit`
* Remove the following feature (no longer available as compilation options):
    - `node-alias` - Replaced by the `circuit-template` feature
* Remove the `splinter node alias` subcommands `add`, `delete`, `list`, and
  `show`. This functionality will be replaced by the `splinter circuit template`
  subcommands in an upcoming release.
* Update `splinter circuit create` command to display the newly proposed
  circuit after it is submitted.
* Add the `--node-file` option to the `splinter circuit create` command. This
  option loads a list of nodes from a YAML file, either on the local file
  system or from a remote server with HTTP. You can use this option with (or
  instead of) the `--node` option. This option supports the Splinter node file
  types used by the node registry and the `splinter node alias` commands.
* Update the `CreateCircuitMessageBuilder::add_node` method (which is used by
  the `splinter circuit propose` command) to check for duplicate node IDs and
  endpoints. If a duplicate is detected, an error is returned to the user.
* Update the `splinter` CLI's `CreateCircuitMessageBuilder::add_service` method
  (which is used for adding services specified with the `--service` argument),
  to check for duplicate service IDs.
* Remove the `node-alias` feature (the `splinter node alias` subcommands) from
  the splinter CLI. This will be replaced by the `circuit-template` feature
  (`splinter circuit template` subcommands) .
* Simplify proposing a circuit with `splinter circuit propose` by automatically
  setting the circuit metadata.
* Update the `splinter circuit propose` command to check for duplicate service
  arguments and return an error if one is found.
* Remove the `splinter circuit default` subcommands and all associated code.
  Default values for the circuit management type and service type are now
  configurable with the environment variables `SPLINTER_CIRCUIT_MANAGEMENT_TYPE`
  and `SPLINTER_CIRCUIT_SERVICE_TYPE`. This change simplifies circuit creation
  and lets users set default values with environment variables.
* Change the `splinter circuit show` command to require a circuit ID as an
  argument.
* Increase the paging limit for `splinter circuit list` to 1000.

### Canopy

* Remove Canopy from the splinter repository and move it to
  https://github.com/Cargill/splinter-ui

### Documentation

* Add man pages for the `splinter circuit` subcommands and `splinter database migrate`.
  To display a man page with the `man` command, use the dashed form of the name,
  where each space is replaced by a dash.
    - `splinter-circuit`
    - `splinter-circuit-list`
    - `splinter-circuit-proposals`
    - `splinter-circuit-propose`
    - `splinter-circuit-show`
    - `splinter-circuit-template`
    - `splinter-circuit-template-arguments`
    - `splinter-circuit-template-list`
    - `splinter-circuit-template-show`
    - `splinter-circuit-vote`
    - `splinter-database-migrate`

### Miscellaneous

* Update the examples
  [private_counter](https://github.com/Cargill/splinter/tree/v0.3.15/examples/private_counter)
  and [private_xo](https://github.com/Cargill/splinter/tree/v0.3.15/examples/private_xo)
  to be compatible with Splinter version 0.3.15.

## Changes in Splinter 0.3.14

### Highlights

* Service ID must now conform to a specific format (a 4-character base-62 string),
  which is enforced by the `SplinterServiceBuilder::build` method when provided
  and randomly generated if one is not.
* Circuit ID must now conform to a specific format (an 11-character string composed
  of two 5-character base-62 strings, joined with a `-`), which is enforced by the
  `CreateCircuitBuilder::build` method when provided and randomly generated if one is not.

### Deprecations

* `splinter::transport::raw` has been deprecated in favor of `splinter::socket::tcp`
* `splinter::transport::tls` has been deprecated in favor of `splinter::socket::tls`

### libsplinter:
* Reorganize the socket-based transports, raw (now renamed tcp) and TLS, under a
  `transport::socket` module.
* Rename `RawTransport` to `TcpTransport` to better reflect the underlying capability.
  Also rename the `socket::raw` module to `socket::tcp`.
* Add a `comments` field to the `Circuit` object to allow for human-readable comments.
* Save a disconnected connection in a `HashMap` to be returned in case the connection
  is removed again at a later time.
* Separate circuit and proposal REST API responses from the internal structs so
  the internal structures can change as needed without impacting the data exposed
  via the REST API.
* Add a `BiHashMap` to `Mesh` for a unique ID to the mesh ID.
* Allow `BiHashMap` look-ups with elements that implement the `Borrow` trait.
* Replace the `RecvError::InternalError` with a `Disconnected` error for accuracy.
* Experimental feature “circuit-template” (new): Implement `CircuitTemplateManager`
  with functionality to list and load available templates.
* For all experimental Biome features:
    - Remove Arc wrappings from Biome’s user store to allow mutable references of
      the user store.
    - Separate Biome models and schemas into their respective modules.
    - Add a `/biome/verify` REST API endpoint to verify a user’s password.
    - Update the Biome Rust API documentation to remove explicit references to
      Rust features.
    - Make the Biome user module private.
    - Rename `biome::datastore` to `biome::migrations` and add module-level
      documentation for biome migrations.
    - Remove all uses of `super` imports in Biome.
* Correct `json-web-tokens` feature guards.
* Experimental feature “proposal-read”:
    - Update the `ProposalStore::proposals` arguments to take a `ProposalFilter`
      enum to allow the proposal store to filter returned proposals.
    - Rename the `/admin/proposals` endpoint's `filter` query parameter to `management_type`.
    - Add `member` filter query parameter to the `GET /admin/proposals` REST API endpoint.

### CLI:
* Experimental feature “circuit”:
    - Update the `splinter circuit create` command to generate a service ID automatically.
    - Add a `comments` argument to the `splinter circuit create` command to optionally
      provide comment for the proposed circuit.
    - Display comments in the output of the `splinter proposals` command.
    - Update the `circuit-read` and `proposal-read` response deserializers to reflect
      the REST API response objects, rather than the related internal structs.
    - Add a `member` argument to the `splinter circuit proposals` command for
      filtering proposals by the given member.
    - Add `SPLINTER_REST_API_URL` environment variable to be used if `-U` or `--url`
      is not specified.
    - Remove the `required` specification from `splinter circuit list` and
      `splinter circuit proposals` arguments that are not actually required.
* Experimental feature “circuit-template” (new): Implement the experimental
  `circuit-template` feature. This feature lets you use a template to create a
   circuit with the new `template` subcommand for `splinter circuit create`.
    - Add the `dry_run` flag to the `circuit create` command, which displays the
      circuit definition without submitting the proposal.
    - Add subcommands behind the `circuit-template` feature to display circuit templates:
      `splinter circuit template list` and `splinter circuit template show`.

### packaging:
* Publish the Splinter package to crates.io when the repository is tagged.
* Add pandoc to the `splinter-dev` image.
* Update the version of `splinter-dev` to v2 and update Dockerfiles to use `splinter-dev:v2`.
* Experimental feature "circuit-template" (new): Include a `scabbard` template with
  the Splinter CLI and include the `gameroom` template with the gameroom daemon.

## Changes in Splinter 0.3.13

### Highlights

* Breaking change: Socket-based transports (TCP and TLS) now require a version
  handshake when connecting.
* New experimental feature `circuit-template`: Circuit template library to
  simplify circuit creation.

### Deprecations and Breaking Changes

For upgrade information, see [Upgrading to Splinter 0.3.13 from
Splinter 0.3.12](https://github.com/Cargill/splinter-docs/blob/master/docs/upgrading/splinter-v0.3.13-from-v0.3.12.md).

* Socket-based transports, such as TCP and TLS, require a version handshake when
  connecting. This handshake allows the pair to negotiate the header version for
  messages sent over the connection. The V1 header currently consists of a
  version, a payload length, and a header checksum.
* `TlsConnection::new` is deprecated. A `TlsConnection` should only be created
  via `TlsTransport`.

### libsplinter

* `RestApi::run` now returns a bind error if it fails to bind to an address.
  `RestApiServerError` has an additional variant, `BindError`.  Matching on
  specific errors will require a new statement for this variant, unless a
  catch-all statement is used.
* Experimental feature "circuit-template" (new): The circuit::template module
  provides library code for building tools to create circuits using templates.
* All experimental Biome features:
    - New OpenAPI documentation for REST API routes.
    - Improved Rust documentation; see
      [docs.rs/splinter](https://docs.rs/splinter/).
    - Restructured data stores to follow new model that enables future
      database selections.
* Experimental feature "biome-key-management": Added response bodies to the
  key management REST API routes.
* Experimental feature "biome-user":
  - Added response bodies to the user REST API routes.
  - Added `/biome/users/{user_id}/keys/{public_key}` endpoint, with both GET and
    DELETE handlers to fetch a specific user by public key and delete a user,
    respectively.
* Experimental feature “json-web-tokens” (new): Moved the sessions and secrets
  modules from behind the biome experimental feature to the REST API module,
  behind the “json-web-tokens” experimental feature
* Experimental feature "connection-manager":
    - Remove `CmResponse` from the Connector API in order to provide a more
      idiomatic Rust API.
    - Automatic reconnection for broken connections.

### CLI

* Experimental feature "circuit": The `splinter circuit create` subcommand now
  displays the circuit ID for a new circuit.

## Changes in Splinter 0.3.12

### Highlights:
* REST APIs and clients are now versioned with a ProtocolVersion.
* OPTIONS/CORS is now supported by the splinterd REST API
* [CanopyJs](https://github.com/Cargill/splinter-canopyjs) and [SaplingJS](https://github.com/Cargill/splinter-saplingjs) have been moved to their own repos.
* The scabbard CLI now provides experimental list and show commands for uploaded
  smart contracts.
* The scabbard REST API now has an experimental get state route
* Numerous bug fixes and documentation improvements.

### Deprecations and Breaking Changes:
  For information on upgrading from 0.3.11 to 0.3.12 see [Upgrading](https://github.com/Cargill/splinter-docs/blob/master/docs/upgrading/splinter-v0.3.12-from-v0.3.11.md) documentation
* Added “admin” prefix to circuits, key registry and node registry REST API
  routes.
* Rewrote the circuit create command so it does not accept a circuit definition
  in a YAML file, but rather it creates the circuit definition based on CLI
  arguments.
* Removed the deprecated generate-cert feature. Now you must use the `splinter
  cert generate` command to create development certificates and keys for using
  the TLS transport. See the how-to for [Generating Insecure Certificates for
  Development](https://github.com/Cargill/splinter-docs/blob/master/docs/howto/generating_insecure_certificates_for_development.md) for more information.
* Changed `scabbard upload` command to `scabbard contract upload` and updated
  .scar file to be loaded by name and version from a specified `--path`
  argument.
* Updated the version of Sawtooth Sabre used by scabbard to v0.5.

### libsplinter:
* Add “admin” prefix to circuits route, key registry and node registry routes.
* Add optional protocol headers to REST APIs. The protocol guard looks for an
  optional header, SplinterProtocolVersion. If this is provided, it will return
  a BadRequest if the version provided is out of range.
* Add protocol guards to the Scabbard REST API endpoints
* Add protocol guards to the Admin service REST API endpoints
* Add protocol guards to the splinterd REST API endpoints
* Add protocol version guards to biome endpoints
* Add openapi documentation for the Biome routes.
* Wait full time in scabbard batch status check.
* Add a CircuitStore trait. This trait will be the external interface for access
  circuit information. This trait was also implemented for SplinterState
  directly.
* Update the circuit routes to use CircuitStore instead of an
  Arc<RwLock<SplinterState>> directly.
* Include node_id in circuit vote error message when the node is not allowed to
  vote for a node on the proposal.
* Validate the requestor's public key length when acting on a circuit proposal
  or vote in the AdminService.
* Reorganize the admin REST API and move circuit REST API into the
  admin::rest_api module.
* Improve shutdown of event reactor by removing the delay on shutdown caused by
  the event::Reactor using the combination of a running flag and reacting to
  that flag based on the, as well as signalling and joining shutdown in the
  same function (delaying other shutdown activities).
* Remove unwrap in libsplinter storage module. This error was propagated to the
  CLI and made debugging difficult.
* Add better documentation updates for circuit-read. Includes fixing the openapi
  documentation for the circuit routes, as well as adding rust doc comments to
  the route implementations.
* Add ServiceArgValidator and implement the trait for Scabbard. The admin
  services will now validate that a circuit proposal has valid service
  arguments, using the new, experimental, trait ServiceArgValidator.
* Add the CircuitFilter, for use as strongly-typed filter parameters to the
  circuits function on CircuitStore.
* Update the sawtooth, sawtooth-sdk, and transact dependencies to the latest
  versions.
* Remove sawtooth-sdk dependency from the scabbard client; it is no longer
  needed with updates to transact.
* Update the scabbard client's ServiceId struct to allow creating directly from
  circuit and service IDs, and to check that circuit and service ID are
  non-empty when parsing from a string.

### splinterd:
* Add a ClapPartialConfigBuilder object, used to construct a PartialConfig
  object from Clap argument values, available behind the `config-command-line`
  feature flag.
* Add a DefaultPartialConfigBuilder object, used to construct a PartialConfig
  object from default values, available behind the `config-default` feature
  flag.
* Add an EnvPartialConfigBuilder object, used to construct a PartialConfig
  object from configuration values defined as environment variables, available
  behind the `config-env-var` feature flag.
* Separate the TomlConfig object from the PartialConfigBuilder implementation as
  this object is now used to define the valid format for a configuration toml
  file.
* Renamed the previous PartialConfigBuilder implementation for the TomlConfig to
  TomlPartialConfigBuilder.
* Add the ConfigBuilder object, which takes in PartialConfigs and then
  constructs a Config object from the values set in the PartialConfig objects.
* Add the Config object; it is used to hold configuration variables compiled
  from several PartialConfig builder objects as well as the source of each
  respective value.
* Simplify logging for the Config object to display the raw configuration
  variables defined in the Config object.
* Added more robust logging for file operations, logging the fully qualified
  path of a file anytime a file is used.
* Remove the deprecated generate-cert feature. Now you must use the
  `splinter cert generate` command to create development certificates and keys
  for using the TLS transport.
* Clean up the main function in splinterd, specifically in how the Config
  objects are used.
* Handle OPTIONS and CORS Requests in splinterd. Respond to an HTTP OPTIONS
  request by returning all of the allowed methods. Also add support for
  handling CORS requests, guarded by a rust feature "rest-api-cors".  This
  checks the preflight conditions of the request, and fails the request if the
  preflight conditions are not met.


### splinter CLI:
* Update the output of the `circuit` and `proposal` subcommands to default to a
  human readable format.
* Add experimental `node alias` subcommand to save node information locally.
  This information can be used to simplify creating circuits.
* Add experimental `circuit default` commands to save local defaults for
  service-types and management type. This information can be used to simplify
  creating circuits.
* Rename keygen `--admin` to `--system` to better reflect the functionality of
  the flag, which is to generate keys for use by a splinter node.
* Send the ADMIN_PROTOCOL_VERSION with the SplinterProtocolVersion header when
  making admin client requests.
* Set the log level for low level crates to Warn to reduce the noise when using
  the cli with -vv
* Rewrite the circuit create command so it does not accept a circuit definition
  in a YAML file, but rather it creates the circuit definition based on CLI
  arguments.
* Add improved help text for `splinter cert generate`.

## scabbard CLI
* Add `GET /state` and `GET /state/{address}` endpoints for scabbard, behind the
  experimental `scabbard-get-state` feature.
* Add contract list/show subcommands to scabbard CLI. `scabbard list` will list
  the name, versions and owners of deployed contracts. `scabbard show` will
  print out the name version, inputs, outputs and who created the contract.
* Send the SCABBARD_PROTOCOL_VERSION with the SplinterProtocolVersion header
  when making scabbard client requests.
* Fix "execute" feature name in cfg's.
* Eliminate recursion in scabbard client's wait. Update the scabbard client's
  `wait_for_batches` function to use a loop rather than recursion to avoid stack
  overflows.
* Use new transaction/batch building pattern enabled by sabre/sawtooth/transact
  to simplify submitting transactions.
* Load .scar files using transact's new .scar file loading functionality; this
  changes the `contract upload` command to take a path and a contract
  name/version as arguments rather than a file path for the .scar file.

### health service:
* Add rest-api feature to Cargo toml. This fixes a bug where it could not build
  if built outside of the workspace.

### Gameroom:
* Fix games disappearing on refresh. Adds a check to ensure that games are not
  refreshed if selectedGameroom state is empty.
* Remove the vuex-module-decorators dependency, which was causing issues with
  debugging and provides little benefit.
* Implement a new component, Loading, which renders a spinner and a message
  supplied by a prop. This standardizes the approach to loading indicators
  throughout gameroom.
* Update the vuex page loading store to store a message.
* Trigger a loading indicator when pages are being lazy loaded or data has to be
  fetched before the page can fully load
* Update gameroom daemon to actix 2.0.
* Remove unnecessary Pike namespace permissions from setting up the XO contract
  in the gameroom daemon.
* Use new transaction/batch building pattern enabled by sabre/sawtooth/transact
  to simplify submitting transactions to scabbard.
* Update the Sabre version for transactions submitted by the gameroom web app to
  match the Sabre version used by scabbard (v0.5).

### Gameroom cli:
* Change gameroom cli version to match the rest of the repo

### Packaging:
* Add curl to the scabbard-cli docker image to enable fetching remote .scar
  files.
* Add scabbard to splinter-dev dockerfile

## Changes in Splinter 0.3.11

### Highlights:
* Splinter supports dynamic multi-party circuit creation in scenarios where the
peers are not connected when the circuit proposal is submitted.
* A new scabbard CLI provides experimental subcommands to submit batches of
transactions to a scabbard service.
* New experimental endpoints have been added to get state from a scabbard
service.
* Information on how to run the Gameroom demo using Kubernetes is available in
[docker/kubernetes/README.md](https://github.com/Cargill/splinter/blob/master/docker/kubernetes/README.md).


### libsplinter:
- Enable more than two-party circuit connection. The admin service now waits
for all nodes in the circuit proposal to be peered before handling a consensus
proposal.
- Add Biome (user management) routes for fetching, listing and deleting users
and updating credentials.
- Establish connection to peers when handling votes. If the connection between
nodes is dropped after a proposal is submitted, a node will try to re-establish
the connection before submitting the vote.
- Add support to configure the timeout value for two-phase commit.
- Set the default timeout for admin and scabbard services to 30 seconds.
- Add experimental endpoints (behind the "scabbard-get-state" feature) for
fetching and listing entries in scabbard state.

### splinterd:
- Replace the current Config object with a PartialConfig and a
PartialConfigBuilder. This is the first part of significantly refactoring how
the Splinter daemon is configured.
- Fix the panic caused by unwrapping a bad protobuf message

### Gameroom:
- Add example files and instructions (in docker/kubernetes) on how to run the
Gameroom demo using Kubernetes.

### scabbard CLI:
- Add the scabbard CLI with experimental subcommands that closely resemble
those of the sabre CLI in Sawtooth Sabre.

### splinter CLI:
- Add a keygen subcommand to generate a user's public/private key pair. This
subcommand uses the "keygen" experimental feature.

### Packaging:
- Update the libsplinter Cargo.toml file to include only experimental and
stable in the list of features in package.metadata.docs.rs.
- Add the Gameroom CLI to the splinter-dev docker image.
- Publish Docker images built with experimental features during nightly cron
builds.

## Changes in Splinter 0.3.10

### libsplinter:
- Fix the wait for batch status in the ScabbardClient by adding the base URL if
  one is not provided.
- Add comments to ScabbardClient tests.

### splinterd:
- Add the CLI flag --enable-biome to the splinterd CLI.  The addition of this
  flag relaxes the requirement that the database-url option must be set.
  Currently, the database-url option is only required for the Biome subsystem.

### Gameroom:
- Add migrations for the Gameroom database to support upgrading an existing
  gameroom daemon.

### Gameroom CLI:
- Add gameroom CLI, which will initially be used to migrate the gameroom
  database.

## Changes in Splinter 0.3.9

### Highlights:
* A Splinter crate is now available at https://crates.io/crates/splinter.
* The "splinter" CLI has new experimental subcommands:
  - splinter circuit create (create a circuit)
  - splinter circuit vote (vote on a circuit)
  - splinter circuit list (display a list of circuits)
  - splinter circuit proposals (display a list of circuit proposals)
  - splinter circuit show (display circuit and proposal details)

### Deprecations and Breaking Changes:
* The new node registry format is enforced as of release 0.3.8. Each entry in
  the node registry must specify a node ID, a display name, and an endpoint.

### libsplinter:
* Add experimental list and fetch routes for circuit proposals to the admin
  service REST API.
* Add key management routes to the Biome REST API, including POST, GET and
  PATCH  to /biome/users/{user_id}/keys.
* Fix Scabbard initial events by rejecting an empty last_seen_event query
  parameter and properly handling empty transaction receipt stores.
* Add a newline to key files generated by the splinter CLI.
* Start initial implementation of ScabbardClient, which provides a convenient
  way to submit batches to a scabbard instance.

### splinterd
* Add experimental Biome users API routes to splinterd.
* Add experimental list and fetch routes for circuit proposals to the admin
  service REST API.
* Log a debug message instead of a warning when the splinterd config file is
  not found.
* Remove panics that can be caused by a user from Splinter daemon startup.
* Fix typos and standardize capitalization in splinterd help and error messages.

### splinter CLI
* Implement experimental "circuit create" subcommand.
* Implement experimental "circuit vote" subcommand.
* Add experimental "circuit list" and "circuit show" subcommands.
* Add "splinter circuit proposals" subcommand to list proposals.
* Add support for showing proposal details to "circuit list" subcommand.
* Change verbose and quiet flags to be global.

### Gameroom
* Add a docker-compose file that uses published images. This compose file can be
  used along with the repository in situations where building from scratch is
  not feasible.
* Update the Gameroom README with CARGO_ARGS instructions for running with
  experimental features.

### Packaging
* Update splinterd packaging for the current node registry format.
* Log current git HEAD commit hash during docker image builds.
* Add a description to the Cargo.toml files.

## Changes in Splinter 0.3.8

### Highlights:

* A new "experimental" feature set.  Features marked as experimental are
  available for use, but are subject to change.
* The “splinter-cli” command has been renamed to “splinter”.

### Deprecations and Breaking Changes:

* The Splinter CLI name has changed from “splinter-cli” to “splinter”.
  “splinter-cli” remains as an alias for “splinter”, but should be considered
  deprecated as of this release.
* The node registry trait was extensively updated to support iteration,
  implementation-agnostic filtering, and simplified modifications.

### Libsplinter:

* The protobuf files are now under the libsplinter subdirectory.  This enables
  publishing the "splinter" library crate.
* NodeRegistry::list_nodes now returns an iterator.
* NodeRegistry::update_node and add_node have been merged to
  NodeRegistry::insert.
* The REST API implementations for circuits, node registry, and key registry
  have moved from splinterd to libsplinter.
* A new "rest-api" stable feature includes the library functions mentioned
  above.
* New Biome features for user-related functionality such as credentials and key
  management. (Biome is the libsplinter module that supports user management.)
    - "biome"
    - "biome-credentials"
    - "biome-key-management"
    - "biome-notifications"
* A new "experimental" feature set that includes all experimental features.

## Changes in Splinter 0.3.7

### Highlights:
* The admin service and the scabbard service can now send catch-up events to
  bring new subscribers up to date

### Deprecations and Breaking Changes:
* The splinterd --generate-certs flag, which was deprecated in 0.3.6, is still
  available by default. In 0.3.8, the flag will not be available by default.
  Instead, you must use the Rust compile-time feature “generate-certs” to
  explicitly enable the deprecated --generate-certs flag. For more information,
  see the 0.3.6 release notes.
* In the next release, the splinter CLI name will change from “splinter-cli” to
  “splinter”. “splinter-cli” will exist as an alias for “splinter”, but should
  be considered deprecated as of release 0.3.8.

### libsplinter:
* Refactor the admin service and scabbard service to separate the REST API code
* Change admin service events to include a timestamp of when the event occurred
* Update the admin service to send all historical events that have occurred
  since a given timestamp when an app auth handler subscribes
* Update the scabbard event format to correlate directly with a transaction
  receipt
* Remove EventHistory from the REST API because it is no longer used
* Remove EventDealer from the REST API and replace it with the EventSender
* Update the event sender to send catch-up events as an asynchronous stream
* Add state-delta catch-up to the scabbard service, sending all events that
  occurred since a given event ID when a subscription request is received
* Update Network to properly clean up connections on disconnection

### splinterd:
* Fix the splinterd --heartbeat argument to properly accept a value

### Gameroom example:
* Update the gameroomd app auth handler to track the timestamp of the last-seen
  admin event
* Add the timestamp for the last-seen admin event to the app auth handler’s
  subscription request for getting any catch-up events
* Update the gameroom state-delta subscriber to track the ID of the last-seen
  scabbard event
* Add the ID of the last-seen scabbard event when subscribing to scabbard on
  restart, which lets the gameroom daemon receive catch-up events

### Private XO example:
* Replace the transact git repo dependency with a crates.io dependency

## Changes in Splinter 0.3.6

### Highlights:
* Peers can now successfully reconnect after restarting
* Faster build times:
  * Added a Dockerfile for splinter-dev docker image
  * Based the installed images on the splinter-dev image
  * Updated the compose files to build the installed docker images
  * Added parallelization to Travis CI builds
* Initial database structure for Biome, the libsplinter module that supports
  user management
* New Gameroom Technical Walkthrough document that explains the Splinter
  functionality that powers the Gameroom application; see
  examples/gameroom/README.md for a link to the PDF

### Deprecated Features:
* The --generate-certs flag for splinterd is now deprecated. Instead, please
  generate development certificates and keys using the new command "splinter-cli
  cert generate". This command will generate the certificates and keys in
  /etc/splinter/certs/ (by default) or in the specified directory.
  Note:  --generate-certs is still available by default in 0.3.6. It will be
  turned off in 0.3.7, but will still be available with a Rust compile-time
  feature flag. If using generated certificates, run splinterd with the
  --insecure flag.

### libsplinter:
* Improve logging:
  * Log when a peer is removed
  * Log an event Reactor background thread error on startup
  * Log REST API background thread startup errors immediately, rather than on
    shutdown
  * Log a WebSocket shutdown
  * Log a peer connection initiation
  * Log the configuration used to start splinterd
  * Add timestamp and thread name to log messages
* Return an error when a peer is disconnected
* Allow consensus threads to log error and exit, rather than panic
* Enforce that member, endpoint, and service IDs are unique to a circuit
* Update the example TOML configuration file
* Verify a CircuitManagementPayload message's payload field, header field, and
  payload signature
* Update example circuit files to use correct enum types
* Fix a typo in DurabilityType enum
* Stop the admin service once a shutdown signal is received
* Fix a locking bug that prevented admin service from properly shutting down
* Stop running services upon admin service shutdown
* Include the service definition in service shutdown error
* Update format lint for Rust 1.39
* Add a Splinter PostgreSQL database to be used by Splinter modules
* Decouple EventDealer and EventHistory to allow the storage of events to be
  managed separately from event history
* Change the log levels of received messages and pings/pongs
* Update EventDealers to return error from EventDealer.add_sender method and
  handle errors from EventDealer.dispatch method
* Store AuthServiceEvents in a Mailbox, replacing LocalEventHistory
* Start reorganizing the admin service module
* Store pending changes as transaction receipts in scabbard
* Add a "state_" prefix to variables that refer to the scabbard LMDB backend
  database, which helps distinguish this database from other databases that
  scabbard may maintain
* Run tests behind the "experimental" feature
* Move the zmq-transport feature, which loads the ZMQ dependency, to experimental
* Rename the node registry method create_node add_node, which more accurately
  reflects its functionality
* Update the struct used to build REST API resources to represent multiple
  method and handler pairs for a given resource
* Fix the node registry implementation’s file editing to completely overwrite
  the YAML node registry file rather than append changes
* Add a disconnect listener to Network; this listener is used to close the
  connection when a peer is disconnected from the network
* Register the AuthorizationManager to listen for peer disconnections to clean
  up old state about the disconnected peer

### splinterd:
* Add endpoints for local registry, including:
  * POST /nodes
  * DELETE /nodes/{identity}
  * PATCH /nodes/{identity}
* Move the node registry implementation from splinterd to libsplinter
* Update the struct used to build REST API resources to represent multiple
  method and handler pairs for a given resource
* Run tests behind the experimental feature
* Add /circuits route, available with circuit-read experimental feature,
* Update splinterd to look for certificates and keys in /etc/splinter/certs (by
  default) or the location specified by "--cert-dir" or the environment variable
  SPLINTER_CERT_DIR
* Deprecate the generate-cert flag (will be removed in a future release) now
  that "splinter-cli cert generate" is available

### splinter-cli:
* Add subcommand "cert generate" to generate certificates and keys that can be
  used to run splinterd for development.

### Canopy:
* Add CSS styles for responsive side navigation bar
* Add default color styles to be used in design app
* Add default typography styles and initial typography documentation
* Add CSS class defaults and themes for navigation
* Add structure and initial introduction page for the documentation app
* Add configuration to build theme CSS bundles
* Add the initial structure for a sapling example (an application to extend
  Canopy)
* Implement register and initialize functions for saplings in CanopyJS
* Add lint and unit tests to Travis CI
* Refactor CanopyJS to improve clarity and extensibility
* Implement CanopyJS user

### Gameroom example:
* Add a generic-themed Gameroom app to installed docker-compose file
* Add functions to check for active gamerooms and resubscribe on startup
* Add volumes for /var/lib/splinter to the docker-compose file
* Add timestamp and thread name to log messages
* Remove the hardcoded protocol for octet-stream submission; instead, use a
  relative URL handled by the proxy
* Attempt to reconnect WebSocket clients if a "close" message is received
* Time out WebSocket client connections and attempt to reconnect
* Convert signature hex string to bytes for signing payloads
* Base the test docker image on the splinter-dev docker image
* Fix a bug with cell selection

### Packaging:
* Remove known errors during a .deb package install

## Changes in Splinter 0.3.5

### Highlights:
* Add network-level heartbeats to improve peer connectivity
* Update Gameroom UI to use the WebSocket Secure protocol (wss) when the
  application protocol is HTTPS
* Improve libsplinter tests
* Add code of conduct to README
* Add the command-line option --common-name to splinterd

### Canopy:
* Add initial directory structure for the Canopy project, a web application
  that hosts pluggable applications and tools built on Splinter

### Gameroom example:
* Remove unnecessary logo files
* Update UI to use wss when the application protocol is HTTPS. This fixes an
  issue where the application could not communicate via WebSockets if the
  application was communicating over HTTPS
* Check for batch status after batch is submitted, then wait for batch to be
  committed or invalidated in gameroomd
* Remove member node’s metadata from gameroom propose request payload
* Fetch member node information from splinterd when gameroomd receives a
  gameroom propose request

### libsplinter:
* Add dockerfile for libsplinter crate generation
* Document the limitations for two-phase commit
* Add network-level heartbeats. The network now creates a thread that will send
  a one-way heartbeat to each connected peer every 30 seconds by default.
* Rename libsplinter crate to splinter
* Store the current state root hash for scabbard's shared transaction state in
  order to support restarts
* Simplify where services can be connected. This ensures that a service is
  connected to the first allowed node and that allowed nodes can only have one
  service.
* Remove peers when a node is disconnected


### libsplinter Testing:
* Update key_not_registered test to use a valid circuit
* Rename error_msg to msg in AdminDirectMessage tests
* Correctly set message type to CircuitMessageType::CIRCUIT_DIRECT_MESSAGE in
  AdminDirectMessage tests
* Fix typos in doc comments

## Changes in Splinter 0.3.4

### Highlights
* Implement a batch status endpoint for scabbard
* Set up the Cypress integration test framework for the Gameroom UI

### Gameroom example
* Copy Splinter .proto files into installed client builds
* Redirect the user to a “Not Found” page if the page does not exist
* Set up integration tests using Cypress
* Add XO smart contract to installed gameroomd builds

### libsplinter
* Reduce latency of events by replacing run_interval in EventDealerWebSocket
  with streams

### scabbard
* Change the scabbard database name to be the sha256 hash of
  service_id::circuit_id to ensure that it will be a valid file name
* Add signature and structure verification to the scabbard service when it
  receives batches submitted via the REST API
* Add /batch_statuses endpoint to scabbard and update /batches endpoint to
  return a /batch_statuses link for the submitted batch IDs

### splinterd
* Add config builder with toml loading (experimental feature)

### Packaging
* Add Dockerfile to package gameroom UI
* Update packaging for gameroomd and splinterd so modified systemd files are
  not overwritten
* Modify gameroomd and splinterd postinst scripts to add data directories
* Add plumbing to properly version deb packages

## Changes in Splinter 0.3.3

### Highlights
* Add functionality to create and play XO games

### libsplinter
* Add EventHistory trait to EventDealer to allow for new event subscribers to catch
  up on previous events. This trait describes how events are stored.
* Add LocalEventHistory, a basic implementation of EventHistory that stores events
  locally in a queue.
* Add MessageWrapper to be consumed by EventDealerWebsockets, to allow for
  shutdown messages to be sent by the EventDealer
* Enforce that a Splinter service may only be added to Splinter state if the
  connecting node is in its list of allowed nodes
* Add Context object for WebSocket callbacks to assist in restarting WebSocket
  connections
* Add specified supported service types to the service orchestrator to determine
  which service types are locally supported versus externally supported
* Only allow initialization of the orchestrator’s supported service
* On restart, reuse the services of circuits which are stored locally
* Add circuit ID when creating a service factory, in case it is needed by the
  service
* Replace UUID with service_id::circuit_id, which is guaranteed to be unique on
  a Splinter node, to name the Scabbard database
* Fix clippy error in events reactor
* Fix tests to match updated cargo args format
* Change certain circuit fields from strings to enums
* Remove Splinter client from CLI to decrease build time

### Gameroom Example
* Add ability in the UI to fetch and list XO games
* Correct arguments used to fetch the members of an existing gameroom, allowing
  the members to be included in the /gamerooms endpoint response
* Add GET /keys/{public_key} endpoint to gameroomd, to fetch key information
  associated with a public key
* Add UI functionality to create a new XO game:
* Add ability to calculate addresses
* Add methods to build and sign XO transactions and batches
* Add methods to submit XO transactions and batches
* Add form for user to create new game
* Add new game notification to UI and gameroomd
* Add player information displayed for a game in UI
* Implement XO game board in UI
* Implement XO take functionality and state styling in UI
* Add component to show game information in the Gameroom details page in the UI
* Use md5 hash of game name when creating a game, rather than URL-encoded name
  that handles special characters
* Add player information when updating an XO game from exported data (from state
  delta export)
* Add auto-generated protos for the UI
* Remove the explicit caching in the Gameroom Detail view in the UI, because Vue
  does this automatically
* Make various UI styling fixes
* Remove unused imports to avoid cargo compilation warnings

## Changes in Splinter 0.3.2

### Highlights
* Completed the code to propose, accept, and create a gameroom in the Gameroom
  example application

### libsplinter
* Persist AdminService state that includes the pending circuits
* Replace the WebSocketClient with a new events module, which improves
  multi-threaded capabilities of the clients (libsplinter::events; requires the
  use of "events" feature flag)
* Improve log messages by logging the length of the bytes instead of the bytes
  themselves
* Fix issue with sending and receiving large messages (greater than 64k)
* Fix issues with threads exiting without reporting the error
* Removed inaccurate warn log message that said signature verification was not
  turned off

### splinterd
* Add Key Registry REST API resources
* Increase message queue sizes for the admin service's ServiceProcessor.

### splinter-cli
* Remove outdated CLI commands

### Gameroom Example
* Add XoStateDeltaProcessor to Gameroom application authorization handler
* Add route to gameroom REST API to submit batches to scabbard service
* Set six-second timeout for toast notifications in the UI
* Add notification in the UI for newly active gamerooms
* Enhance invitation UI and add tabs for viewing sent, received, or all
  invitations
* Fix bug that caused read notifications to not appear as read in the UI
* Fix bug where the Gameroom WebSocket was sending notifications to the UI
  every 3 seconds instead of when a new notification was added

## Changes in Splinter 0.3.1

### Highlights

* Completion of circuit proposal validation, voting, and dynamic circuit creation
* Addition of key generation and management, as well as role-based permissions
* Continued progress towards proposing, accepting, and creating a gameroom in the
  Gameroom example application

### libsplinter

* Add AdminService, with support for:
  * Accepting and verifying votes on circuit proposals
  * Committing approved circuit proposals to SplinterState
* Add notification to be sent to application authorization handlers when a
  circuit is ready
* Update scabbard to properly set up Sabre state by adding admin keys
* Add support for exposing service endpoints using the orchestrator and service
  factories
* Add WebSocketClient for consuming Splinter service events
* Add KeyRegistry trait for managing key information with a StorageKeyRegistry
  implementation, backed by the storage module
* Add KeyPermissionsManager trait for accessing simple, role-based permissions
  using public keys and an insecure AllowAllKeyPermissionManager implementation
* Add SHA512 hash implementation of signing traits, for test cases
* Add Sawtooth-compatible signing trait implementations behind the
  "sawtooth-signing-compat" feature flag.

### splinterd

* Add package metadata and license field to Cargo.toml file
* Add example configuration files, systemd files, and postinst script to Debian
  package
* Reorder internal service startup to ensure that the admin service and
  orchestrator can appropriately connect and start up
* Use SawtoothSecp256k1SignatureVerifier for admin service

### splinter-cli

* Add "splinter-cli admin keygen" command to generate secp256k1 public/private
  key pairs
* Add "splinter-cli admin keyregistry" command to generate a key registry and
  key pairs based on a YAML specification

### Private XO and Private Counter Examples
* Add license field to all Cargo.toml files
* Rename private-xo package to private-xo-service-<version>.deb
* Rename private-counter packages to private-counter-cli-<version>.deb and
  private-counter-service-<version>.deb

### Gameroom Example
* Add package metadata and license field to gameroomd Cargo.toml file
* Add example configs, systemd files, and postinst script to gameroomd Debian
  package; rename package to gameroom-<version>.deb
* Implement notification retrieval using WebSocket subscription and
  notifications endpoints
* Show pending and accepted gamerooms in the Gameroom UI
* Add full support for signing CircuitManagementPayloads with the user's
  private key and submitting it to splinterd
* Update gameroomd to specify itself as the scabbard admin and submit the XO
  smart contract when the circuit is ready
* Make various UI enhancements

## Changes in Splinter 0.3.0

### Highlights

* Completion of the two-phase commit consensus algorithm with deterministic
  coordination
* Continued progress towards dynamically generating circuits, including
  dynamic peering and circuit proposal validation
* Continued progress on the Gameroom example, including UI updates and
  automatic reconnection

### libsplinter

* Add a service orchestration implementation
* Add Scabbard service factory
* Implement a deterministic two-phase commit coordinator
* Reorder the commit/reject process for the two-phase commit coordinator. The
  coordinator now tells proposal manager to commit/reject before broadcasting
  the corresponding message to other verifiers.
* Refactor two-phase commit complete_coordination. Move the process of
  finishing the coordination of a proposal in two-phase commit to a single
  function to reduce duplication.
* Implement a two-phase commit timeout for consensus proposals
* Update the two-phase commit algorithm to ignore duplicate proposals
* Allow dynamic verifiers for a single instance of two-phase commit consensus
* Add an Authorization Inquisitor trait for inspecting peer authorization state
* Add the ability to queue messages from unauthorized peers and unpeered nodes
  to the admin service
* Fix an issue that caused the admin service to deadlock when handling proposals
* Add Event Dealers for services to construct websocket endpoints
* Add a subscribe endpoint to Scabbard
* Validate circuit proposals against existing Splinter state
* Update create-circuit notification messages to include durability field

### splinterd

* Log only warning-level messages from Tokio and Hyper
* Improve Splinter component build times
* Add a NoOp registry to handle when a node registry backend is not specified

### Private XO and Private Counter Examples

* Use service IDs as peer node IDs, in order to make them compatible with
  two-phase consensus

### Gameroom Example

* Add server-side WebSocket notifications to the UI
* Add borders to the Acme UI
* Improve error handling and add reconnects to the Application Authorization
  Handler
* Add a circuit ID and hash to GET /proposals endpoint
* Standardize buttons and forms in the UI
* Improve error formatting in the UI by adding toasts and progress bar spinners
* Change the Gameroom REST API to retrieve node data automatically on startup
* Split the circuit_proposals table into gameroom and gameroom_proposals tables
* Use the [Material elevation strategy](https://material.io/design/color/dark-theme.html)
  for coloring the UI
* Decrease the font size
* Change the UI to redirect users who are not logged in to login page
* Add a dashboard view
* Add an invitation cards view
* Add a button for creating a new gameroom to the UI

## Changes in Splinter 0.2.0

### libsplinter

* Add new consensus API (libsplinter::consensus)
* Add new consensus implementation for N-party, two-phase commit
  (libsplinter::consensus::two_phase)
* Add new service SDK with in-process service implementations
  (libsplinter::service)
* Add initial implementation for Scabbard, a Splinter service for running Sabre
  transactions with two-phase commit between services
(libsplinter::service::scabbard)
* Add REST API SDK (consider this experimental, as the backing implementation
  may change)
* Add new node registry REST API endpoint for providing information about all
  possible nodes in the network, with initial YAML-file backed implementation.
* Add new signing API for verifying and signing messages, with optional
  Ursa-backed implementation (libsplinter::signing, requires the use of
"ursa-compat" feature flag)
* Add MultiTransport for managing multiple transport types and selecting
  connections based on a URI (libsplinter::transport::multi)
* Add ZMQ transport implementation (libsplinter::transport::zmq, requires the
  use of the "zmq-transport" feature flag)
* Add peer authorization callbacks, in order to notify other system entities
  that a peer is fully ready to receive messages


### splinterd

* Add REST API instance to provide node registry API endpoints
* Add CLI parameter --bind for the REST API port
* Add CLI parameters for configuring node registryy; the default registry type
  is "FILE"

### Gameroom Example

* Add gameroom example infrastructure, such as the gameroomd binary, docker
  images, and compose files
* Add Login and Register UI
* Add New Gameroom UI
* Add UI themes for both parties in demo
* Initialize Gameroom database
* Add circuit proposals table
* Initialize Gameroom REST API
* Implement Gameroom REST API authentication routes
* Implement Gameroom REST API create gameroom endpoint
* Implement Gameroom REST API proposals route
* Implement /nodes endpoint in gameroomd
