% SPLINTER-CIRCUIT-ABANDON(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-circuit-abandon** — Submits a request to abandon the specified circuit.

SYNOPSIS
========
**splinter circuit abandon** \[**FLAGS**\] \[**OPTIONS**\] CIRCUIT-ID

DESCRIPTION
===========
Request to abandon a circuit by specifying the circuit ID of the circuit to be
abandoned. Abandoning a circuit  will remove the circuit's networking ability
for the abandoning node. This will also notify other circuit members that the
circuit has been abandoned, but does not require further action from other
circuit members.

The generated ID of the existing circuit can be viewed using the
`splinter-circuit-list` command and this circuit ID is used to specify the
circuit to be abandoned.

This operation is not able to be performed on circuits that are not active at
the time of the request. As this operation does disconnect the specified circuit
from its networking capability for the abandoning node, other circuit members
should take care to notice when the circuit is abandoned so as not to attempt to
communicate with this circuit member, as this abandoning circuit member has
disabled the circuit's routing capability from their end. This removes the
nodes ability to communicate over this circuit.

FLAGS
=====
`-h`, `--help`
: Prints help information

`-V`, `--version`
: Prints version information

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
* The existing circuit has ID `1234-ABCDE`.

The following command displays a member node requesting to abandon the circuit:
```
$ splinter circuit abandon \
  --key MEMBER-NODE-PRIVATE-KEY-FILE \
  --url URL-of-member-node-splinterd-REST-API \
  1234-ABCDE \
```

ENVIRONMENT
===========
**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| `splinter-circuit-disband(1)`
| `splinter-circuit-list(1)`
| `splinter-circuit-purge(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.5/
