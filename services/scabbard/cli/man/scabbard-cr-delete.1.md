% SCABBARD-CR-DELETE(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2020 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**scabbard-cr-delete** — Deletes a Sabre contract registry.

SYNOPSIS
========

**scabbard cr delete** \[**FLAGS**\] \[**OPTIONS**\] NAME

DESCRIPTION
===========
This command allows users to delete a Sabre contract registry from the targeted
scabbard service.

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
: Provides the name of the contract registry to delete.

EXAMPLES
========
The following command removes the `xo` contract registry from a scabbard service
on circuit `01234-ABCDE` with service ID `abcd`, running on the node with the
REST API endpoint `http://localhost:8088`. The transaction will be signed with
the key located in the file `~/user.priv`.

```
$ scabbard cr delete \
  --url http://localhost:8088 \
  --service-id 01234-ABCDE::abcd \
  --key ~/user.priv \
  xo
```

The next command removes the `intkey_multiply` contract registry from the same
scabbard service, but specifies a key in the `$HOME/.splinter/keys` directory by
name. It also waits up to 10 seconds for the contract registry deletion batch to
commit.

```
$ scabbard cr delete \
  --url http://localhost:8088 \
  --service-id 01234-ABCDE::abcd \
  --key user \
  --wait 10 \
  intkey_multiply
```

SEE ALSO
========
| `scabbard-cr-create(1)`
| `scabbard-cr-update(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/
