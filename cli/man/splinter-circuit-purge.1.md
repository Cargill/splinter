% SPLINTER-CIRCUIT-PURGE(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-circuit-purge** — Submits a request to purge the specified circuit.

SYNOPSIS
========
**splinter circuit purge** \[**FLAGS**\] \[**OPTIONS**\] CIRCUIT-ID

DESCRIPTION
===========
Request to purge a circuit by specifying the circuit ID of the circuit to be
removed from the node's storage. A circuit is only available to be purged
if it has already been disbanded and are only available locally. Disbanding a
circuit removes a circuit's networking functionality.

The generated ID of the existing disbanded circuit can be viewed using the
`splinter-circuit-list`, with the `--circuit-status` option of `disbanded`.

The purge request is only available for members of the node, as the circuit is
only available to the node locally. If the circuit has not been disbanded, it
is not able to be purged. Once a circuit has been purged, it is removed from
the node's storage and is no longer viewable.

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
: Specify the circuit ID of the circuit to be purged.

EXAMPLES
========
* The existing disbanded circuit has ID `1234-ABCDE`.

The following command displays a member node requesting to purge the circuit:
```
$ splinter circuit purge \
  --key MEMBER-NODE-PRIVATE-KEY-FILE \
  --url URL-of-member-node-splinterd-REST-API \
  1234-ABCDE \
```

ENVIRONMENT VARIABLES
=====================
**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| `splinter-circuit-abandon(1)`
| `splinter-circuit-disband(1)`
| `splinter-circuit-list(1)`
| `splinter-circuit-show(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.5/
