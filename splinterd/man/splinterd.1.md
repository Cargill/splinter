% SPLINTERD(1) Cargill, Incorporated | Splinter Commands

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
as `splinterd.toml`). For examples, see
[splinterd/packaging/splinterd.toml.example](https://github.com/Cargill/splinter/blob/master/splinterd/packaging/splinterd.toml.example)
and
[splinterd/sample_configs](https://github.com/Cargill/splinter/tree/master/splinterd/sample_configs)
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

FLAGS
=====

`--enable-biome`
: Enable the Biome subsystem, which provides user management functions for
  Splinter applications. The `--database` option is required when this flag is
  used.

`-h`, `--help`
: Prints help information.

`--no-tls`
: Turns off TLS configuration and restricts `splinterd` to TCP (`raw`)
  connections. This flag allows `splinterd` to start without the certificates
  and keys that TLS requires. Without `--no-tls`, if `splinterd` cannot find the
  certificates and keys required by TLS, it exits with an error.

`-q`, `--quiet`
: Decreases verbosity (the opposite of `-v`). When specified, only errors or
  warnings will be displayed.

`--tls-insecure`
: Turns off certificate authority validation for TLS connections; all peer
  certificates are accepted. This flag is intended for development environments
  using self-signed certificates.

`-V`, `--version`
: Prints version information.

`-v`
: Increases verbosity (the opposite of -q). Specify multiple times for more
  output.

OPTIONS
=======

`--admin-timeout TIMEOUT`
: Sets the coordinator timeout, in seconds, for admin service proposals.
  (Default: 30 seconds.)

  This setting affects consensus-related activities for pending circuit changes
  (functions that use the two-phase commit agreement protocol in the Scabbard
  service).

`--advertised-endpoint` `ADVERTISED-ENDPOINT`
: Specifies the public network endpoint for daemon-to-daemon communication
  between Splinter nodes, if the network endpoint is not public. Use the format
  `tcp://ip:port`. (Default: Same as the network endpoint; see
  `-n`, `--network-endpoint`.)

`--bind BIND-ENDPOINT`
: Specifies the connection endpoint for the REST API. (Default: 127.0.0.1:8080.)

`-c`, `--config` `CONFIG-FILE`
: Specifies the path and file name for a `splinterd` configuration file, which
  is a TOML file that contains `splinterd` settings. (The file name must end
  with a `.toml` extension.) For examples, see
  [splinterd/packaging/splinterd.toml.example](https://github.com/Cargill/splinter/blob/master/splinterd/packaging/splinterd.toml.example)
  and
  [splinter/splinterd/sample_configs](https://github.com/Cargill/splinter/tree/master/splinterd/sample_configs)
  in the `splinter` repository.

  Any options on the command line will override the settings in the
  configuration file.

`--database DB-URL`
: Specifies the URL for the PostgreSQL database used for Biome. (Default:
  127.0.0.1:5432.) This option is required when `--enable-biome` is used.

`--heartbeat SECONDS`
: Specifies how often, in seconds, to send a heartbeat. (Default: 30 seconds.)
  Use 0 to turn off the heartbeat.

  This heartbeat is used to check the health of connections to other Splinter
  nodes.

`-n`, `--network-endpoint` `NETWORK-ENDPOINT`
: Specifies the endpoint for daemon-to-daemon communication between Splinter
  nodes, using the format `tcp://ip:port`. (Default: 127.0.0.1:8044.)

  `--node-id NODE-ID`
: (Required) Sets a new ID for the node. The node ID must be unique across the
  network (for all Splinter nodes that could participate on the same circuit).

`--peer PEER-URL` `[,...]`
: Specifies one or more Splinter nodes that `splinterd` will automatically
  connect to when it starts. The *PEER-URL* argument must specify another node's
  network endpoint, using the format `ip:port`.

  Specify multiple nodes in a comma-separated list or by repeating the
  `--peer` option.

`--registry REGISTRY-FILE` `[,...]`
: Specifies one or more read-only node registry files.

`--service-endpoint SERVICE-ENDPOINT`
: Specifies the endpoint for service-to-daemon communication, using the format
  `tcp://ip:port`. (Default: `127.0.0.1:8043`.)

`--storage STORAGE-TYPE`
: Specifies whether to store circuit state in memory or in a local YAML file.
  *STORAGE-TYPE* can be `memory` or `yaml` (the default). By default, this
  file is stored in `/var/lib/splinter` (unless `SPLINTER_STATE_DIR`
  is set).

  Using `memory` for storage means that circuits will not persist when
  `splinterd` restarts.

`--tls-ca-file CERT-FILE`
: Specifies the path and file name for the trusted CA certificate.
  (Default: `/etc/splinter/certs/ca_pem`.)

  Do not use this option with the `--tls-insecure` flag.

`--tls-cert-dir CERT-DIR`
: Specifies the directory that contains the trusted CA certificates and
  associated key files. (Default: `/etc/splinter/certs/`.)

  This option overrides the `SPLINTER_CERT_DIR` environment variable, if set,
  and any setting in the `splinterd` configuration file.

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

`--whitelist WHITELIST` `[,...]`
: Lists one or more trusted domains for cross-origin resource sharing (CORS).
  This option allows the specified domains to access restricted web resources
  in a Splinter application.  If this option is not specified, all domains will
  be allowed to access Splinter web resources.

  Specify multiple domains in a comma-separated list or with separate
  `--whitelist` options.

CERTIFICATE FILES
=================

When the Splinter daemon runs in TLS mode (using `tcps` connections at the
transport layer), it requires certificate authority (CA) certificates and
associated keys that are stored in `/etc/splinter/certs/` by default.

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
Development](https://github.com/Cargill/splinter-docs/blob/master/docs/howto/generating_insecure_certificates_for_development.md)"
in the Splinter documentation.

ENVIRONMENT VARIABLES
=====================

**SPLINTER_CERT_DIR**
: Specifies the directory containing certificate and associated key files.
  (See `--tls-cert-dir`.)

**SPLINTER_STATE_DIR**
: Specifies where to store the circuit state YAML file, if `--storage` is
  set to `yaml`. (See `--storage`.) By default, this file is stored in
  `/var/lib/splinter`.

FILES
=====
`/etc/splinter/certs/`
: Default location for CA certificates and keys

`/var/lib/splinter/`
: Default location for the circuit state YAML file.

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
# Setting heartbeat_interval to 0 disables this feature.
heartbeat_interval = 60
```

SEE ALSO
========
| `splinter-circuit-propose(1)`
| `splinter-cert-generate(1)`
|
| Splinter documentation: https://github.com/Cargill/splinter-docs/blob/master/docs/index.md
