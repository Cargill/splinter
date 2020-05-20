% SCABBARD-CONTRACT-LIST(1) Cargill, Incorporated | Splinter Commands

NAME
====

**scabbard-contract-list** — Displays a scabbard service's smart contracts

SYNOPSIS
========

**scabbard contract list** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========
This command allows users to list all smart contracts that have been uploaded to
a particular scabbard service. The smart contract details are displayed in three
columns: `NAME`, `VERSIONS`, and `OWNERS`. This command can be used to verify
that one or more contracts have been uploaded, or to discover what contracts are
available on the given scabbard service. Because scabbard services share state
with each other, all services on the same circuit will have the same contracts.

FLAGS
=====
`-h`, `--help`
: Prints help information.

`-v`
: Increases verbosity. Specify multiple times for more output.

OPTIONS
=======
`-f`, `--format` FORMAT
: Specifies the output format of the listed smart contracts. (default `human`).
  Possible values for formatting are `human` and `csv`, where `human` displays
  information in a table.

`--service-id` ID
: Specifies the fully-qualified service ID of the targeted scabbard service,
  using the format `CIRCUIT_ID::SERVICE_ID`. This option is required.

`-U`, `--url` URL
: Specifies the URL for the `splinterd` REST API that is running the targeted
  scabbard service. (default `http://localhost:8080`) This option is required.

EXAMPLES
========
The following command lists the smart contracts uploaded to a scabbard service
on circuit `01234-ABCDE` with service ID `abcd`, running on the node with the
REST API endpoint `http://localhost:8088`.

```
$ scabbard contract list \
  --url http://localhost:8088 \
  --service-id 01234-ABCDE::abcd
NAME VERSIONS OWNERS
xo   0.3.3    0385d50a3512f1ef324c9fc86798998d4e3ad2a4e189ceb9ca49aacdcad30a595a
```

The next command displays the smart contracts from the same service, but
the output is formatted as CSV.

```
$ scabbard contract list \
  --url http://localhost:8088 \
  --service-id 01234-ABCDE::abcd \
  --format csv
NAME,VERSIONS,OWNERS
xo,0.3.3,0385d50a3512f1ef324c9fc86798998d4e3ad2a4e189ceb9ca49aacdcad30a595a
```

SEE ALSO
========
| `scabbard-contract-show(1)`
| `scabbard-contract-upload(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/
