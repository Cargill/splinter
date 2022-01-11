% SPLINTER-CIRCUIT(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2022 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-circuit** — Provides circuit management functionality.

SYNOPSIS
========
**splinter circuit** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========
This command allows nodes to create and manage circuits and circuit proposals.
Commands to list and display circuits and circuit proposals that a node is a member
of are also available subcommands. Nodes are also able to vote to accept or reject
circuit proposals using the `splinter-circuit-vote` subcommand.

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

SUBCOMMANDS
===========

`abandon`
: Abandon an existing circuit.

`disband`
: Propose to disband an existing circuit.

`list`
: List all circuits that have been accepted by all proposed members.

`proposals`
: List all circuit proposals. Circuit proposals have not been voted on by all
  proposed members.

`propose`
: Propose a new circuit to be created.

`purge`
: Purge an existing inactive circuit.

`remove-proposal`
: Remove a circuit proposal.

`show`
: Display a specific circuit or circuit proposal.

`template`
: Manage circuit templates used for circuit creation.

`vote`
: Vote on a new circuit proposal. Only the proposed members that did not propose
  the circuit are able to vote on a circuit. The circuit requester has an assumed
  vote of `ACCEPT`.

SEE ALSO
========
| `splinter-circuit-abandon(1)`
| `splinter-circuit-disband(1)`
| `splinter-circuit-list(1)`
| `splinter-circuit-proposals(1)`
| `splinter-circuit-propose(1)`
| `splinter-circuit-purge(1)`
| `splinter-circuit-remove-proposal(1)`
| `splinter-circuit-show(1)`
| `splinter-circuit-template-arguments(1)`
| `splinter-circuit-template-list(1)`
| `splinter-circuit-template-show(1)`
| `splinter-circuit-vote(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.7/
