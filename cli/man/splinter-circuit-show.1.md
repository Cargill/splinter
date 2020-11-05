% SPLINTER-CIRCUIT-SHOW(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2020 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-circuit-show** — Displays information about a circuit

SYNOPSIS
========
**splinter circuit show** \[**FLAGS**\] \[**OPTIONS**\] CIRCUIT

DESCRIPTION
===========
Display the entire definition of a circuit or proposal that the node is a member
or proposed member of. All members, or proposed members, may view the circuit
definition. Viewing a proposed circuit enables nodes to view information pertaining
to other proposed member nodes as well as the status of a node’s vote regarding
the circuit. The proposed circuit will be viewable unless any proposed member nodes
reject the circuit proposal.

FLAGS
=====
`-h`, `--help`
: Prints help information

`-q`, `--quiet`
: Decrease verbosity (the opposite of -v). When specified, only errors or
  warnings will be output.

`-V`, `--version`
: Prints version information

`-v`
: Increases verbosity (the opposite of -q). Specify multiple times for more
  output.

OPTIONS
=======
`-F`, `--format` FORMAT
: Specifies the output format of the circuit proposal. (default `human`).
  Possible values for formatting are `human` and `csv`.

`-k`, `--key` PRIVATE-KEY-FILE
: Specifies the private signing key (either a file path or the name of a
  .priv file in $HOME/.splinter/keys).

`-U`, `--url` URL
: Specifies the URL for the `splinterd` REST API. The URL is required unless
  `$SPLINTER_REST_API_URL` is set.


ARGUMENTS
=========
`CIRCUIT`
: Specify the circuit ID of the circuit to be shown.

EXAMPLES
========
This command displays information about a circuit with the default `human`
formatting, which intends to use indentation and labels to make the circuit
information understandable.

* The proposing node has ID `alpha001` and endpoint
  `tcps://splinterd-node-alpha001:8044`, and a service ID of `AA01`.
* The proposed member node has ID `beta001` and endpoint
  `tcps://splinterd-node-beta001:8044`, and a service ID of `BB01`.
* The command shows a circuit that was proposed by node alpha001 but has yet to
  be voted on by beta001, with a circuit ID of `01234-ABCDE`.

The information displayed below will appear the same on all proposed member nodes.
If all member nodes vote to accept the circuit, the `splinter-circuit-show`
command will display the same information, without the `Vote` as all nodes would
have accepted the proposal. If any of the member nodes vote to reject the circuit,
the proposal will not be viewable by any nodes.

```
$ splinter circuit show 01234-ABCDE \
  ---url URL-of-alpha-node-splinterd-REST-API
Proposal to create: 01234-ABCDE
    Management Type: mgmt001

    alpha-001 (tcps://splinterd-node-alpha001:8044)
        Vote: ACCEPT (implied as requester):
            ALPHA-PUBLIC-KEY
        Service: AA01
            admin_keys:
                ALPHA-PUBLIC-KEY
            peer_services:
                BB01

    beta-001 (tcps://splinterd-node-beta001:8044)
        Vote: PENDING
        Service: AA01
            admin_keys:
                ALPHA-PUBLIC-KEY
            peer_services:
                AA01
```

ENVIRONMENT VARIABLES
=====================
**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| `splinter-circuit-propose(1)`
| `splinter-circuit-list(1)`
| `splinter-circuit-proposals(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.5/
