% TRANSACT-PLAYLIST-PROCESS(1) Cargill, Incorporated | Transact Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**transact-playlist-processe** — Processes a transaction playlist

SYNOPSIS
========
**transact playlist create ** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========
Processes a transaction playlist. A playlist is a series of transactions,
described in YAML.  This command processes a playlist, converting it into
transactions and writes it to file or standard out.

`transact-playlist-batch` takes the output file and creates signed batches
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
`-i, --input FILE`
: The source of the input playlist yaml.

`--k, --key FILE`
: The signing key for the transactions.

`-o, --output FILE`
: The target for the generated transactions.

`--workload WORKLOAD`
: The workload type to the playlist is for. [possible values: smallbank]


EXAMPLES
========
The following shows creating a file, `txns.text`

```
transact playlist process \
  -i smallbank.yaml \
  --key ./alice.priv \
  --output txns.text \
  --workload smallbank
```


SEE ALSO
========
| `transact(1)`
| `transact-playlist(1)`
| `transact-playlist-create(1)`
| `transact-playlist-batch(1)`
| `transact-playlist-submit(1)`
|
| Transact documentation: https://docs.rs/transact/latest
