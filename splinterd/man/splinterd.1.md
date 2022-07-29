% SPLINTERD(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2022 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinterd** — Starts the Splinter daemon

SYNOPSIS
========

**splinterd** \[**FLAGS**\] \[**OPTIONS**\] <arguments>

DESCRIPTION
===========

This command configures and runs the Splinter daemon, `splinterd`. This daemon
provides core Splinter functionality such as circuit management, a REST API,
and an admin service. It can also include application-specific services.

The `splinterd` command requires a new ID for the node. Other settings are
optional.

**Configuration File**

This command provides the `-c` and `--config` options to specify an optional
TOML configuration file containing `splinterd` settings, instead of using
command-line options. (Any options on the command line will override those in
the configuration file.) The file name must end with a `.toml` extension, such
as `splinterd.toml`). For an example, see
[splinterd/packaging/splinterd.toml.example](https://github.com/Cargill/splinter/blob/main/splinterd/packaging/splinterd.toml.example)
in the `splinter` repository.

**Connection Types**

The Splinter daemon supports transport-level connections with raw (TCP),
Transport Layer Security (TLS), and WebSocket protocols, which are also called
"transport types". A node can use multiple transport types at once. By default,
`splinterd` configures all transport types. If the `--no-tls` flag is set, no
TLS connections are configured.


TLS requires certificate authority (CA) certificates and keys, which are stored
in `/etc/splinter/certs/` by default. If necessary, you can change the
directory, specify individual file paths and names, or use self-signed
(insecure) certificates in a development environment. For more information, see
the certificate-related options and CERTIFICATE FILES, below.

**Directory Locations**

This command includes several options that change default Splinter directory
locations. These directory locations can also be changed with environment
variables or settings in the `splinterd` TOML configuration file. For more
information, see `--config-dir`, `--tls-cert-dir`, and
"SPLINTER DIRECTORY PATHS", below.

FLAGS
=====

`--enable-biome-credentials`
: Enables Biome credentials for REST API authentication.

`--disable-scabbard-autocleanup`
: Disable autocleanup of pruned scabbard merkle state.

`-h`, `--help`
: Prints help information.

`--no-tls`
: Turns off TLS configuration and restricts `splinterd` to TCP (`raw`)
  connections. This flag allows `splinterd` to start without the certificates
  and keys that TLS requires. Without `--no-tls`, if `splinterd` cannot find the
  certificates and keys required by TLS, it exits with an error.

`--tls-insecure`
: Turns off certificate authority validation for TLS connections; all peer
  certificates are accepted. This flag is intended for development environments
  using self-signed certificates.

`-V`, `--version`
: Prints version information.

`-v`
: Increases verbosity. Specify multiple times for more output.

OPTIONS
=======

`--admin-timeout TIMEOUT`
: Sets the coordinator timeout, in seconds, for admin service proposals.
  (Default: 30 seconds.)

  This setting affects consensus-related activities for pending circuit changes
  (functions that use the two-phase commit agreement protocol in the Scabbard
  service).

`--advertised-endpoints` `ADVERTISED-ENDPOINT`
: Specifies the public network endpoint for daemon-to-daemon communication
  between Splinter nodes, if the network endpoint is not public. Use the format
  `tcp://ip:port`. (Default: Same as the network endpoint; see
  `-n`, `--network-endpoints`.)

  Specify multiple endpoints in a comma-separated list or with separate
  `--advertised-endpoint` options.

`-c`, `--config` `CONFIG-FILE`
: Specifies the path and file name for a `splinterd` configuration file, which
  is a TOML file that contains `splinterd` settings. (The file name must end
  with a `.toml` extension.) For an example, see
  [splinterd/packaging/splinterd.toml.example](https://github.com/Cargill/splinter/blob/main/splinterd/packaging/splinterd.toml.example)
  in the `splinter` repository.

  Any options on the command line will override the settings in the
  configuration file.

`--config-dir CONFIG-DIR`
: Specifies the directory containing Splinter configuration files. (Default:
  `/etc/splinter`, unless `SPLINTER_CONFIG_DIR` or `SPLINTER_HOME` is set.)

`--display-name DISPLAY-NAME`
: Specifies a human-readable name for the node (Default: "Node NODE-ID")

`--database DB-URL`
: Specifies the URL or connection string for the PostgreSQL or SQLite database
  used for Splinter state, including circuits, proposals and Biome. (Default:
  SQLite database splinter_state.db) This option is required. The default SQLite
  database will go in the directory, `/var/lib/splinter`, unless
  `SPLINTER_STATE_DIR` or `SPLINTER_HOME` is set.

  Using `memory` or `:memory:` as the DB-URL means that state will not
  persist when `splinterd` restarts.

`--heartbeat SECONDS`
: Specifies how often, in seconds, to send a heartbeat. (Default: 30 seconds.)
  Use 0 to turn off the heartbeat.

  This heartbeat is used to check the health of connections to other Splinter
  nodes.

`--influx-db` `DB_NAME`
: The name of the InfluxDB database for metrics Collection.

`--influx-password` `PASSWORD`
: The password used for authorization with the InfluxDB.

`--influx-url` `URL`
: The URL to connect the InfluxDB database for metrics collection.

`--influx-username` `USERNAME`
: The username used for authorization with the InfluxDB.

`--lifecycle-executor-interval` `interval`
: How often the lifecycle executor should be woken up to check for pending
  services, in seconds. (Default: 30)

`-n`, `--network-endpoints` `NETWORK-ENDPOINT`
: Specifies the endpoint for daemon-to-daemon communication between Splinter
  nodes, using the format `protocol_prefix://ip:port`.
  (Default: tcps://127.0.0.1:8044.)

  Specify multiple endpoints in a comma-separated list or with separate
  `-n` or `--network-endpoint` options.

  `--node-id NODE-ID`
: (Required) Sets a new ID for the node. The node ID must be unique across the
  network (for all Splinter nodes that could participate on the same circuit).

`--oauth-client-id OAUTH-CLIENT-ID`
: Specifies the client ID for the OAuth provider used by the REST API.

`--oauth-client-secret OAUTH-CLIENT-SECRET`
: Specifies the client secret for the OAuth provider used by the REST API.

`--oauth-openid-auth-params` `[,...]`
: Specifies one or more additional parameters to add to OAuth OpenID auth
  requests. Each parameter must be formatted as a `<key>=<value>` pair. This
  option only has an effect when `--oauth-provider openid` is used.

`--oauth-openid-scopes` `[,...]`
: Specifies one or more additional scopes to request from the OAuth OpenID
  provider. This option only has an effect when `--oauth-provider openid` is
  used.

`--oauth-openid-url OAUTH-OPENID-URL`
: OpenID discovery document URL for the OAuth provider used by the REST API.
  This option is required when `--oauth-provider azure` or
  `--oauth-provider openid` is used.

`--oauth-provider OAUTH-PROVIDER`
: Specifies the OAuth provider used by the REST API. Accepted values: `azure`,
  `github`, `google`, `openid`.

`--oauth-redirect-url OAUTH-REDIRECT-URL`
: Redirect URL for the OAuth provider used by the REST API.

`--peers PEER-URL` `[,...]`
: Specifies one or more Splinter nodes that `splinterd` will automatically
  connect to when it starts. The *PEER-URL* argument must specify another node's
  network endpoint, using the format `protocol_prefix://ip:port` or
  `protocol-prefix+trust://ip:port` to require trust authorization. Default
  authorization type is challenge if signing keys are configured.

  Specify multiple nodes in a comma-separated list or by repeating the
  `--peers` option. The protocol prefix part of the peer URL specifies the
  type of connection that is created.

`--peering-key PEERING_KEY`
: The name of the key to use for challenge authorization with specified peers.
  Defaults to the only key if there is only one key supported otherwise,
  defaults to `splinterd`. This key is expected to be present in the storage
  directory.

`--registries REGISTRY-FILE` `[,...]`
: Specifies one or more read-only Splinter registry files.

`--registry-auto-refresh SECONDS`
: Specifies how often, in seconds, to fetch remote node registry changes in the
  background. (Default: 600 seconds.) Use 0 to turn off automatic refreshes.

`--registry-forced-refresh SECONDS`
: Specifies how often, in seconds, to fetch remote node registry changes on
  read. (Default: 10 seconds.) Use 0 to turn off forced refreshes.

`--rest-api-endpoint REST-API-ENDPOINT`
: Specifies the connection endpoint for the REST API. (Default: 127.0.0.1:8443.)

`--scabbard-state SCABBARD-STATE`
: Specifies where scabbard stores its internal state. Accepted values: `lmdb`,
  `database`

`--service-timer-interval INTERVAL`
: How often the service timer should be woken up, in seconds
  (Default: 1)

`--state-dir STATE-DIR`
: Specifies the storage directory.
  (Default: `/var/lib/splinter`.)

  This option overrides the `SPLINTER_STATE_DIR` environment variable, if set.

`--tls-ca-file CERT-FILE`
: Specifies the path and file name for the trusted CA certificate.
  (Default: `/etc/splinter/certs/ca.pem`.)

  Do not use this option with the `--tls-insecure` flag.

`--tls-cert-dir CERT-DIR`
: Specifies the directory that contains the trusted CA certificates and
  associated key files. (Default: `/etc/splinter/certs/`, unless
  `SPLINTER_CERT_DIR` or `SPLINTER_HOME` is set).

`--tls-client-cert CERT-FILE`
: Specifies the path and file name for the client certificate, which is
  used by `splinterd` when it is sending messages over TLS. (Default:
  `/etc/splinter/certs/client.crt`.)

`--tls-client-key CLIENT-KEY`
: Specifies the path and file name for the client key.
  (Default: `/etc/splinter/certs/client.key`.)

`--tls-server-cert SERVER-CERT`
: Specifies the path and file name for the server certificate, which is used by
  `splinterd` when it is receiving messages over TLS.
  (Default: `/etc/splinter/certs/server.crt`.)

`--tls-server-key SERVER-KEY`
: Specifies the path and file name for the server key.
  (Default: `/etc/splinter/certs/server.key`.)

`--tls-rest-api-cert REST-API-CERT`
: Specifies the path and file name for the REST API certificate, which is used by
  `splinterd` when it is hosting the REST API over HTTPS.
  (Default: `/etc/splinter/certs/rest_api.crt`.)

`--tls-rest-api-key REST-API-KEY`
: Specifies the path and file name for the REST API key.
  (Default: `/etc/splinter/certs/rest_api.key`.)

`--allow-list ALLOW_LIST` `[,...]`
: Lists one or more trusted domains for cross-origin resource sharing (CORS).
  This option allows the specified domains to access restricted web resources
  in a Splinter application.  If this option is not specified, all domains will
  be allowed to access Splinter web resources.

  Specify multiple domains in a comma-separated list or with separate
  `--allow-list` options.

CERTIFICATE FILES
=================

When the Splinter daemon runs in TLS mode (using `tcps` connections at the
transport layer), it requires certificate authority (CA) certificates and
associated keys that are stored in `/etc/splinter/certs/` by default
(or `$SPLINTER_HOME/certs` if that environment variable is set).

You can change the certificate directory by using the `--tls-cert-dir` option,
setting the `SPLINTER_CERT_DIR` environment variable, or specifying the
location in a `splinterd` configuration file.

By default, the following file names are used for the certificates and
associated keys:

* `ca.pem`
* `client.crt`
* `client.key`
* `server.crt`
* `server.key`

You can specify different paths and file names with the `--tls-ca-file`,
`--tls-client-cert`, `--tls-client-key`, `--tls-server-cert`, and
`--tls-server-key` options (or related settings in the configuration file).

In a development environment, you can use the `--tls-insecure` flag to use
self-signed certificates and keys (which can be generated by the
`splinter cert generate` command). For more information, see
"[Generating Insecure Certificates for
Development](https://www.splinter.dev/docs/0.7/howto/generating_insecure_certificates_for_development.html)"
in the Splinter documentation.

SPLINTER DIRECTORY PATHS
========================

Several Splinter directories have the following default locations:

* Splinter configuration directory: `/etc/splinter`

* State directory: `/var/lib/splinter/`

* TLS certificate directory: `/etc/splinter/certs/`

For the configuration and certificate directories, the directory paths can be
changed individually with a `splinterd` option, a setting in a TOML config file,
or an environment variable. For more information, see `--config-dir`,
`--database` and `--tls-cert-dir`.

In addition, the `SPLINTER_HOME` environment variable provides a simple way to
change the base path for all of these directories. This variable is intended for
development and testing. When `SPLINTER_HOME` is set, the default directory
paths are:

* Splinter configuration directory: `$SPLINTER_HOME/etc/`

* State directory: `$SPLINTER_HOME/data/`

* TLS certificate directory: `$SPLINTER_HOME/certs/`

For example, if `SPLINTER_HOME` is set to `/tmp/testing`, the default path for
the Splinter state directory is `/tmp/testing/data/`.

Note: If an individual environment variable is set, it overrides the value in
`SPLINTER_HOME` (if also set) for that directory. For example, `SPLINTER_HOME`
is set to `/tmp/testing` and `SPLINTER_STATE_DIR` is set to
`/tmp/splinter/state`, the Splinter state directory is `/tmp/splinter/state`.

AUTHORIZATION CONFIGURATION
===========================

Currently, splinterd supports three authorization types: Biome credentials,
Cylinder JWT, and OAuth.

Cylinder JWT authorization is enabled by default.

A Cylinder JWT is a custom JSON Web Token implementation which supports the
signing algorithms provided by the Cylinder library. This includes secp256k1
which is currently used for signing transactions and Splinter administrative
payloads. This allows the same signing key to be used as an authorization
identity.

Biome credentials can be enabled using the `--enable-biome-credentials` flag.

The Splinter daemon provides 5 options for configuring OAuth for the REST API:

* `oauth-provider` for specifying the OAuth provider that splinterd will use to
  get the client's identity. Currently, `azure`, `github`, `google`, and
  `openid` are supported.

* `oauth-client-id` for specifying the client ID, which is a public identifier
  for an app that's registered with the chosen OAuth provider.

* `oauth-client-secret` for specifying the client secret, which is a private
  value known only to the OAuth provider and the application.

* `oauth-redirect-url` for specifying the endpoint that the OAuth provider will
  redirect to when completing authentication.

* `oauth-openid-url` for specifying the OpenID discovery document URL that will
  be used to find the OAuth and OpenID endpoints for authentication. This option
  is required if the `oauth-provider` option is set to `azure` or `openid`; if a
  different provider is configured, this option will have no effect.

* `oauth-openid-auth-params` for specifying additional parameters to add to
  OAuth OpenID auth requests. Each parameter must be formatted as a
  `<key>=<value>` pair. This option only has an effect when
  `--oauth-provider openid` is used; if a different provider is configured,
  this option will have no effect.

* `oauth-openid-scopes` for specifying one or more additional scopes to request
  from the OAuth OpenID provider. This option only has an effect when
  `--oauth-provider openid` is used; if a different provider is configured,
  this option will have no effect.

The first 4 of the above arguments (provider, client ID, client secret, and
redirect URL) must be provided when using OAuth authorization. If some but not
all of these 4 arguments are provided, splinterd will fail to start.

The client ID, client secret, and redirect URL must all be registered for an app
with the chosen provider. The client ID and secret are generated by the provider.
The redirect URL should be the `/oauth/callback` endpoint of splinterd's REST
API; this address will depend on the visible address of the REST API. For
example, if the splinterd REST API is available to applications at
`https://www.example.com/`, the redirect URL would be
`https://www.example.com/oauth/callback`.

ENVIRONMENT VARIABLES
=====================

**SPLINTER_CERT_DIR**
: Specifies the directory containing certificate and associated key files.
  (See `--tls-cert-dir`.)

**SPLINTER_CONFIG_DIR**
: Specifies the directory containing configuration files.
  (See: `--config-dir`.)

**SPLINTER_HOME**
: Changes the base directory path for the Splinter directories, including the
  certificate directory. See the "SPLINTER DIRECTORY PATHS" for more
  information.

  This value is not used if an environment variable for a specific directory
  is set (`SPLINTER_CERT_DIR`, `SPLINTER_CONFIG_DIR`, or `SPLINTER_STATE_DIR`).

**SPLINTER_STATE_DIR**
: Specifies where to store the circuit state SQLite database file, if
  `--database` is not set. (See `--database`.) By default, this file is stored
  in `/var/lib/splinter`.

**SPLINTER_STRICT_REF_COUNT**
: Turns on strict peer reference counting. If `SPLINTER_STRICT_REF_COUNT`is set
  to `true` and the peer manager tries to remove a peer reference that does not
  exist, the Splinter daemon will panic. By default, the daemon does not panic
  and instead logs an error. This environment variable is intended for
  development and testing.

**OAUTH_CLIENT_ID**
: Specifies the client ID for the OAuth provider used by the REST API. See
  `--oauth-client-id`.

**OAUTH_CLIENT_SECRET**
: Specifies the client secret for the OAuth provider used by the REST API. See
  `--oauth-client-secret`.

**OAUTH_OPENID_URL**
: URL for the OpenID provider's discovery document used by the REST API. See
  `--oauth-openid-url`.

**OAUTH_PROVIDER**
: Specifies the OAuth provider used by the REST API. See `--oauth-provider`.

**OAUTH_REDIRECT_URL**
: Redirect URL for the OAuth provider used by the REST API. See
  `--oauth-redirect-url`.

FILES
=====

`/etc/splinter/`
: Default location for the Splinter configuration directory. Note: If
  `$SPLINTER_HOME` is set, the default location is `$SPLINTER_HOME/etc/`.

`/etc/splinter/certs/`
: Default location for the TLS certificate directory, which stores CA
  certificates and the associated keys. Note: If `SPLINTER_HOME` is set, the
  default location is `$SPLINTER_HOME/certs/`.

`/var/lib/splinter/`
: Default location for the Splinter state directory, which stores the circuit
  state SQLite database file (unless `--database` is set). Note: If
  `$SPLINTER_HOME` is set, the default location is `$SPLINTER_HOME/data/`.

EXAMPLES
========

To run `splinterd` using the default settings, specify a new, unique node ID.
Replace the example value with the ID for your node.

```
$ splinterd --node-id mynode
```

To specify a configuration file instead of command-line options, use the
`-c` or `--config` option.

```
$ splinterd --config ./configs/splinterd-mynode.toml
```

In this example, the configuration file specifies the node ID and increases
the heartbeat interval to 60 seconds.

```
# Friendly identifier for this node. Must be unique on the network.
node_id = "mynode"

# The number of seconds between network keep-alive heartbeat messages.
# Setting heartbeat to 0 disables this feature.
heartbeat = 60
```

The next example demonstrates how to configure GitHub as an OAuth provider for
REST API authentication, where the placeholder values for the client ID and
secret would be replaced with actual values for a registered GitHub OAuth app:

```
$ splinterd --node-id mynode \
  --oauth-provider github \
  --oauth-client-id <my-client-id> \
  --oauth-client-secret <my-client-secret> \
  --oauth-redirect http://localhost:8080/oauth/callback
```

The above example assumes that the splinterd REST API is accessible to the web
browser at the address `http://localhost:8080/`; if the REST API has a different
address, this argument would be changed accordingly. For example, if the REST
API is proxied to the address `http://localhost:8080/splinterd`, the redirect
URL would be `http://localhost:8080/splinterd/oauth/callback`. If the REST API
is hosted at `https://www.example.com/`, the redirect would be
`https://www.example.com/oauth/callback`.

Similar to the GitHub example, here is how you would configure a Google OAuth
provider for REST API authentication:

```
$ splinterd --node-id mynode \
  --oauth-provider google \
  --oauth-client-id <my-client-id> \
  --oauth-client-secret <my-client-secret> \
  --oauth-redirect http://localhost:8080/oauth/callback
```

Here is how you could configure an Azure OAuth provider:

```
$ splinterd --node-id mynode \
  --oauth-provider azure \
  --oauth-client-id <my-client-id> \
  --oauth-client-secret <my-client-secret> \
  --oauth-redirect http://localhost:8080/oauth/callback \
  --oauth-openid-url https://login.microsoftonline.com/common/v2.0/.well-known/openid-configuration
```

SEE ALSO
========
| `splinter-circuit-propose(1)`
| `splinter-cert-generate(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.7/
