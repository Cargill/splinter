% SCABBARD-PERM(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2020 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**scabbard-perm** — Sets or deletes a Sabre namespace permission.

SYNOPSIS
========

**scabbard perm** \[**FLAGS**\] \[**OPTIONS**\] NAMESPACE CONTRACT

DESCRIPTION
===========
This command allows users to set or delete permissions for Sabre namespaces in
state for the targeted scabbard service. Setting a permission requires the
namespace's state address prefix, the name of the smart contract to set
permissions for, and the `-r`/`--read` and/or `-w`/`--write` flags to indicate
the permissions to set. Deleting a permission requires the namespace's state
address prefix and the `--delete` flag; this deletes all contracts' permissions
for the namespace.

FLAGS
=====
`-d`, `--delete`
: Deletes all permissions for the namespace.

`-h`, `--help`
: Prints help information.

`-r`, `--read`
: Adds namespace read permissions for the contract. This flag conflicts with the
  `-d`/`--delete` flag.

`-v`
: Increases verbosity. Specify multiple times for more output.

`-w`, `--write`
: Adds namespace write permissions for the contract. This flag conflicts with
  the `-d`/`--delete` flag.

OPTIONS
=======
`-k`, `--key` FILE
: Indicates the key file to use for signing scabbard transactions. The `FILE`
  can be a relative or absolute file path, or it can be the name of a .priv file
  in the `$HOME/.splinter/keys` directory. The target file must contain a valid
  secp256k1 private key. This option is required.

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
: Provides the state address prefix of the namespace to set permissions for.

`CONTRACT`
: Specifies the name of the contract to give permissions to for the namespace.
  This argument conflicts with the `-d`/`--delete` flag.

EXAMPLES
========
The following command gives read permissions for the `abcdef` namespace to the
`xo` smart contract in a scabbard service on circuit `01234-ABCDE` with service
ID `abcd`, running on the node with the REST API endpoint
`http://localhost:8088`. The transaction will be signed with the key located in
the file `~/user.priv`.

```
$ scabbard perm \
  --url http://localhost:8088 \
  --service-id 01234-ABCDE::abcd \
  --key ~/user.priv \
  --read \
  abcdef \
  xo
```

The next command gives both read and write permissions for the `012345`
namespace to the `intkey_multiply` smart contract in the same scabbard service.
It also specifies a key in the `$HOME/.splinter/keys` directory by name and
waits up to 10 seconds for the namespace permission batch to commit.

```
$ scabbard perm \
  --url http://localhost:8088 \
  --service-id 01234-ABCDE::abcd \
  --key user \
  --wait 10 \
  --read \
  --write \
  012345 \
  intkey_multiply
```

This example deletes all permissions for the `012abc` namespace in the same
scabbard service as the previous examples.


```
$ scabbard perm \
  --url http://localhost:8088 \
  --service-id 01234-ABCDE::abcd \
  --key ~/user.priv \
  --delete \
  012abc
```

SEE ALSO
========
| `scabbard-ns-create(1)`
| `scabbard-ns-delete(1)`
| `scabbard-ns-update(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/
