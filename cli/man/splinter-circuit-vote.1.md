% SPLINTER-CIRCUIT-VOTE(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2020 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-circuit-vote** — Submits a vote to accept or reject a circuit proposal

SYNOPSIS
========
**splinter circuit vote** \[**FLAGS**\] \[**OPTIONS**\] CIRCUIT-ID --accept --reject

DESCRIPTION
===========
Vote on a new circuit proposal by specifying the circuit ID of the circuit the node
is voting on, as well as the flag to `--accept` or `--reject`. The generated ID of
a proposed circuit can be viewed using the `splinter-circuit-proposals` command and
this circuit ID is used to specify the circuit proposal being voted on. A circuit
proposal is viewable, via the `splinter-circuit-proposals` or `splinter-circuit-show`
commands, by all proposed member nodes unless a proposed member node votes to
reject the circuit proposal. A circuit proposal needs to be voted on by all proposed
members that did not propose the circuit in the first place. Circuit proposers have
an assumed `ACCEPT` vote, as these nodes requested the creation of the circuit.

FLAGS
=====
`--accept`
: Accept the circuit proposal specified.

`-h`, `--help`
: Prints help information.

`-q`, `--quiet`
: Decrease verbosity (the opposite of -v). When specified, only errors or
  warnings will be output.

`--reject`
: Reject the circuit proposal specified.

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
: Specify the circuit ID of the circuit to be voted on.

EXAMPLES
========
* The proposed circuit has ID `1234-ABCDE`.

The following command displays a member node voting to accept the circuit proposal:
```
$ splinter circuit vote \
  --key PROPOSED-MEMBER-NODE-PRIVATE-KEY-FILE \
  --url URL-of-proposed-member-node-splinterd-REST-API \
  1234-ABCDE \
  --accept
```

The following command displays a member node voting to reject the circuit proposal:
```
$ splinter circuit vote \
  --key PROPOSED-MEMBER-NODE-PRIVATE-KEY-FILE \
  --url URL-of-proposed-member-node-splinterd-REST-API \
  1234-ABCDE \
  --reject
```

ENVIRONMENT VARIABLES
=====================
**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| `splinter-circuit-propose(1)`
| `splinter-circuit-proposals(1)`
| `splinter-circuit-show(1)`
|
| Splinter documentation: https://github.com/Cargill/splinter-docs/blob/master/docs/index.md
