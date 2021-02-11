% SPLINTER-CIRCUIT-DISBAND(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-circuit-disband** — Submits a request to disband the specified circuit.

SYNOPSIS
========
**splinter circuit disband** \[**FLAGS**\] \[**OPTIONS**\] CIRCUIT-ID

DESCRIPTION
===========
Request to disband a circuit by specifying the circuit ID of the circuit to be
disbanded. Disbanding a circuit removes a circuit's networking functionality.
Once all members of the circuit have accepted the request to disband the
circuit, the circuit is only available offline. This functionality is currently
behind the experimental `circuit-disband` feature and must be enabled to use
this command.

The `disband` command creates a new circuit proposal to reflect the disbanded
state, with the proposed circuit's `circuit_status` field set to `Disbanded`.
This proposal is then able to be voted on, similar to other circuit proposals.

The generated ID of the existing circuit can be viewed using the
`splinter-circuit-list` command and this circuit ID is used to specify the
circuit to be disbanded. Once the disband request has been submitted,
the proposal created (and other circuit proposals) can be viewed using the
`splinter-circuit-proposals` command.

The disband proposal must be accepted by all members before the existing
circuit is updated to reflect the disbanded state. If all nodes have agreed to
disband the circuit, the disbanded circuit may be viewed using the
`splinter-circuit-show` command.

FLAGS
=====
`-h`, `--help`
: Prints help information.

`-q`, `--quiet`
: Decrease verbosity (the opposite of -v). When specified, only errors or
  warnings will be output.

`-V`, `--version`
: Prints version information.

`-v`
: Increases verbosity (the opposite of -q). Specify multiple times for more
  output.

OPTIONS
=======
`-k`, `--key` PRIVATE-KEY-FILE
: Specifies the full path to the private key file.

`-U`, `--url` URL
: Specifies the URL for the `splinterd` REST API. The URL is required unless
  `$SPLINTER_REST_API_URL` is set.

ARGUMENTS
=========
`CIRCUIT-ID`
: Specify the circuit ID of the circuit to be disbanded.

EXAMPLES
========
* The proposed circuit has ID `1234-ABCDE`.

The following command displays a member node requesting to disband the circuit:
```
$ splinter circuit disband \
  --key PROPOSED-MEMBER-NODE-PRIVATE-KEY-FILE \
  --url URL-of-proposed-member-node-splinterd-REST-API \
  1234-ABCDE \
```

ENVIRONMENT VARIABLES
=====================
**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| `splinter-circuit-abandon(1)`
| `splinter-circuit-list(1)`
| `splinter-circuit-proposals(1)`
| `splinter-circuit-purge(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.5/
