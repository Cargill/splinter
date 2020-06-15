% SCABBARD-CR-CREATE(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2020 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**scabbard-cr-create** — Creates a Sabre contract registry.

SYNOPSIS
========

**scabbard cr create** \[**FLAGS**\] \[**OPTIONS**\] NAME

DESCRIPTION
===========
This command allows users to create a new Sabre contract registry in state for
the targeted scabbard service.

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
: Includes the given public keys as owners of the new contract registry. The
  contract registry must have one or more owners.

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
`NAME`
: Provides the name of the contract registry to create.

EXAMPLES
========
The following command adds the `xo` contract registry to a scabbard service on
circuit `01234-ABCDE` with service ID `abcd`, running on the node with the
REST API endpoint `http://localhost:8088`. The new contract registry has one
owner, and the transaction will be signed with the key located in the file
`~/user.priv`.

```
$ scabbard cr create \
  --url http://localhost:8088 \
  --service-id 01234-ABCDE::abcd \
  --key ~/user.priv \
  --owner 0385d50a3512f1ef324c9fc86798998d4e3ad2a4e189ceb9ca49aacdcad30a595a \
  xo
```

The next command adds the `intkey_multiply` contract registry to the same
scabbard service, but adds multiple owners and specifies a key in the
`$HOME/.splinter/keys` directory by name. It also waits up to 10 seconds for the
contract registry creation batch to commit.

```
$ scabbard cr create \
  --url http://localhost:8088 \
  --service-id 01234-ABCDE::abcd \
  --key user \
  --owner 0385d50a3512f1ef324c9fc86798998d4e3ad2a4e189ceb9ca49aacdcad30a595a \
  --owner 7b6c889058c2d22558ead2c61b321634b74e705c42f890e6b7bc2c80abb4713118 \
  --owner 02381b606ac2bbe3bd374654cb7cb467ffb0225eb46038a5ec37b43e0c2f085dcb \
  --wait 10 \
  intkey_multiply
```

SEE ALSO
========
| `scabbard-cr-delete(1)`
| `scabbard-cr-update(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/
