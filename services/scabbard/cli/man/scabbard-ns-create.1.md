% SCABBARD-NS-CREATE(1) Cargill, Incorporated | Splinter Commands

NAME
====

**scabbard-ns-create** — Creates a Sabre namespace.

SYNOPSIS
========

**scabbard ns create** \[**FLAGS**\] \[**OPTIONS**\] NAMESPACE

DESCRIPTION
===========
This command allows users to create a new Sabre namespace in state for the
targeted scabbard service. A Sabre namespace is a reserved portion of state that
can be written to and read by one or more smart contracts. A contract must be
given permission to read or write to a namespace (see `scabbard-perm(1)` for
setting namespace permissions).

FLAGS
=====
`-h`, `--help`
: Prints help information.

`-v`
: Increases verbosity. Specify multiple times for more output.

OPTIONS
=======
`-k`, `--key` FILE
: Indicates the key file to use for signing scabbard transactions. The `FILE`
  can be a relative or absolute file path, or it can be the name of a .priv file
  in the `$HOME/.splinter/keys` directory. The target file must contain a valid
  secp256k1 private key. This option is required.

`-O`, `--owners` KEY
: Includes the given public keys as owners of the new namespace. The namespace
  must have one or more owners.

`--service-id` ID
: Specifies the fully-qualified service ID of the targeted scabbard service,
  using the format `CIRCUIT_ID::SERVICE_ID`. This option is required.

`-U`, `--url` URL
: Specifies the URL for the `splinterd` REST API that is running the targeted
  scabbard service. (default `http://localhost:8080`) This option is required.

`--wait` SECONDS
: If provided, waits the given number of seconds for the batch to commit.
  Displays an error message if the batch does not commit in time.

ARGUMENTS
=========
`NAMESPACE`
: Provides the state address prefix to reserve for the new namespace.

EXAMPLES
========
The following command creates a new namespace for the `abcdef` address prefix in
a scabbard service on circuit `01234-ABCDE` with service ID `abcd`, running on
the node with the REST API endpoint `http://localhost:8088`. The new namespace
has one owner, and the transaction will be signed with the key located in the
file `~/user.priv`.

```
$ scabbard ns create \
  --url http://localhost:8088 \
  --service-id 01234-ABCDE::abcd \
  --key ~/user.priv \
  --owner 0385d50a3512f1ef324c9fc86798998d4e3ad2a4e189ceb9ca49aacdcad30a595a \
  abcdef
```

The next command creates a new namespace for the `012345` address prefix in the
same scabbard service, but adds multiple owners and specifies a key in the
`$HOME/.splinter/keys` directory by name. It also waits up to 10 seconds for the
namespace creation batch to commit.

```
$ scabbard ns create \
  --url http://localhost:8088 \
  --service-id 01234-ABCDE::abcd \
  --key user \
  --owner 0385d50a3512f1ef324c9fc86798998d4e3ad2a4e189ceb9ca49aacdcad30a595a \
  --owner 7b6c889058c2d22558ead2c61b321634b74e705c42f890e6b7bc2c80abb4713118 \
  --owner 02381b606ac2bbe3bd374654cb7cb467ffb0225eb46038a5ec37b43e0c2f085dcb \
  --wait 10 \
  012345
```

SEE ALSO
========
| `scabbard-ns-delete(1)`
| `scabbard-ns-update(1)`
| `scabbard-perm(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/
