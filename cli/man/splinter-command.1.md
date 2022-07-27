% SPLINTER-COMMAND(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

**splinter-command** — Interacts with the command family smart contract

SYNOPSIS
========
**splinter command** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========
This command can be used to interact with a command family smart contract on a
Splinter circuit with scabbard services.

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
`get-state`
: Submits a Sabre transaction to request a state read of the addresses given.

`help`
:  Prints this message or the help of the given subcommand(s)

`set-state`
: Submits a Sabre transaction to request one or more state write of the state
  entries given. The state entry is a key value pair where the key is a state
  address and the value is the value to be set for the given address.

`show-state`
: Display the state value at the given state address if it exists.

SEE ALSO
========
| `splinter(1)`
| `splinter-command-get-state(1)`
| `splinter-command-set-state(1)`
| `splinter-command-show-state(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.7/
