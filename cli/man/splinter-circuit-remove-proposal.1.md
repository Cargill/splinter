% SPLINTER-CIRCUIT-REMOVE-PROPOSAL(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2022 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-circuit-remove-proposal** — Remove a circuit proposal

SYNOPSIS
========
| **splinter circuit remove-proposal** \[**FLAGS**\] \[**OPTIONS**\] CIRCUIT_ID

DESCRIPTION
===========
Remove a circuit proposal from the node. This command only effects the
requesting member. If any proposed member has removed a circuit proposal, the
proposal may not be voted on to become a circuit. A circuit proposal may be
removed at any point until it has been committed to state as a circuit.

For information on how to remove a circuit, see the `splinter-circuit-disband`,
`splinter-circuit-abandon`, and `splinter-circuit-purge` commands.

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
`-k`, `--key` PRIVATE-KEY-FILE
: Specifies the private signing key (either a file path or the name of a
  .priv file in $HOME/.splinter/keys).

`-U`, `--url` URL
: Specifies the URL for the `splinterd` REST API. The URL is required unless
  `$SPLINTER_REST_API_URL` is set.

ARGUMENTS
=========
`CIRCUIT_ID`
: Specify the circuit ID of the circuit proposal to be removed.

EXAMPLES
========
This command removes a circuit proposal from the requesting node, without
affecting the other proposed members record of the proposal. The following
shows how a circuit proposal with a circuit ID of `01234-ABCDE` is removed.

```
$ splinter circuit remove-proposal 01234-ABCDE \
  --url URL-of-splinterd-REST-API \
  -k path-to-private-key-file
```

You may verify the circuit proposal has been removed for the requesting node
using the `splinter-circuit-proposals` or `splinter-circuit-show` command.

ENVIRONMENT VARIABLES
=====================
**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| `splinter-circuit-abandon(1)`
| `splinter-circuit-disband(1)`
| `splinter-circuit-list(1)`
| `splinter-circuit-proposals(1)`
| `splinter-circuit-propose(1)`
| `splinter-circuit-purge(1)`
| `splinter-circuit-show(1)`
| `splinter-circuit-vote(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.7/
