% SPLINTER-WORKLOAD(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-workload** — Submits a workload against a distributed ledger

SYNOPSIS
========
**splinter workload** \[**FLAGS**\] \[**SUBCOMMAND**\]

DESCRIPTION
===========
This command is intended to be used in performance and stability testing
of distributed ledgers by continuously submitting batches to the provided
target groups at some rate.

The workload tool assumes the distributed ledger's REST API supports Cylinder
JWT authentication.

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
`-d, --duration DURATION`
: Length of time in hours that the workload should run for. If this option is not
  used, the workload will run indefinitely. Duration can be provided in seconds,
  minutes, hours, or days ex: `7s`, `20m`, `24h`, `2d`

`-k, --key PRIVATE-KEY-FILE`
: Specifies the full path to the private key file. The key will be used to
  sign the batches as well as generate a JWT for authentication.

`--smallbank-num-accounts ACCOUNTS`
: The number of smallbank accounts to make. (default: `100`)

`---seed SEEDS`
: An integer to use as a seed to make the workload reproducible.

`--target-rate TARGET-RATE`
: Rate of batch submissions, either provide a float, a batch rate in form 
  <float>/<h,m,s> or a range with the min and max separated by `-` 
  ex: `5.0/s-15.0/m`,`1/m`,`15/s-2/m`,`2.0` (default: `1/s`)

`--targets TARGETS`
:  Node URLS to submit batches to, combine groups with `;`. Each group will get
   batches from a different workload generator to handle dependency
   requirements. The URL should include all of the information required to
   append `/batches` to the end.

`-u, --update UPDATE `
: The time in seconds between updates. The workloads will log the success rate
  of submitting the HTTP requests. (default: `30`)

`--workload WORKLOAD `
: The workload to be submitted. The possible values are `smallbank` or `command`.
  Determines the type of sabre transactions contained within the batches
  submitted by the workload, either smallbank or command payloads.

EXAMPLES
========
The following shows starting 1 workload against a Splinter circuit `jEWSK-jdjSM`
with scabbard services. A Scabbard service runs a sabre transaction handler.
The smallbank smart contract must already be submitted to scabbard.

```
splinter workload \
  --target-rate 1/s \
  --key ./alice.priv \
  --workload smallbank \
  --update 5 \
  --smallbank-seed 10 \
  --smallbank-num-accounts 5 \
  --targets "http://0.0.0.0:8089/scabbard/jEWSK-jdjSM/a001;http://0.0.0.0:8088/scabbard/jEWSK-jdjSM/ \
  --duration 24h
```


SEE ALSO
========
| `splinter(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.7/
