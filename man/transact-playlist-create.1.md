% TRANSACT-PLAYLIST-CREATE(1) Cargill, Incorporated | Transact Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**transact-playlist-create** — Generates a workload transaction playlist

SYNOPSIS
========
**transact playlist create ** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========
This command generates a workload transaction playlist. A playlist is a series
of transactions, described in YAML.  This command generates a playlist and
writes it to file or standard out.

`transact-playlist-process` takes this playlist and creates signed transactions
for the payloads.

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
`-o, --output FILE`
: The target for the generated playlist

`--smallbank-num-accounts ACCOUNTS`
: The number of smallbank accounts to make. (Defaults to 10)

`--smallbank-seed SEED`
: An integer to use as a seed generate the same smallbank playlist

`-n, --transactions NUMBER`
: The number of transactions to generate. This includes the account creation
  payloads. (Defaults to 10)


`--workload WORKLOAD`
:  The workload type to create a playlist for. [possible values: smallbank]


EXAMPLES
========
The following shows creating a smallbank playlist file `smallbank.yaml` with 20
transactions.

```
transact playlist create \
  --smallbank-num-accounts 10\
  --output smallbank.yaml  \
  --smallbank-seed 10 \
  --transactions 20  \
  --workload smallbank
```


SEE ALSO
========
| `transact(1)`
| `transact-playlist(1)`
| `transact-playlist-batch(1)`
| `transact-playlist-process(1)`
| `transact-playlist-submit(1)`
|
| Transact documentation: https://docs.rs/transact/latest
