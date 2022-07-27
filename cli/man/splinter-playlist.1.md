% SPLINTER-PLAYLIST(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

**splinter-playlist** — Create and process playlists of pregenerated payloads

SYNOPSIS
========
**splinter playlist** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========
This command can be used to generate files of pregenerated payloads,
transactions, and batches. The file containing the batches can then be submitted
against a distributed ledger.

Payload, transactions and batch generation can be very expensive and can skew
performance results during testing. Using a pregenerated batch file makes for a
more accurate and repeatable test.

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
`batch`
: Generates signed batches from transaction input. The transaction input is
  expected to be length-delimited protobuf Transaction messages, which should
  also be pre-signed for submission to the validator.

`create`
: Generates a workload transaction playlist. A playlist is a series of
  transactions, described in YAML. This command generates a playlist and writes
  it to file or standard out.

`help`
:  Prints this message or the help of the given subcommand(s)

`process`
: Processes a transaction playlist. A playlist is a series of transactions,
  described in YAML.  This command processes a playlist, converting it into
  transactions and writes it to file or standard out.

`submit`
: Submits signed batches to one or more targets from batch input. The batch
  input is expected to be length-delimited protobuf Batch messages, which
  should also be pre-signed for submission to the validator.

SEE ALSO
========
| `splinter(1)`
| `splinter-playlist-create(1)`
| `splinter-playlist-batch(1)`
| `splinter-playlist-submit(1)`
| `splinter-playlist-process(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.7/
