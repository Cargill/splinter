% SPLINTER-UPGRADE(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-upgrade** — Upgrades splinter YAML state to database state

SYNOPSIS
========

**splinter** **upgrade** \[**FLAGS**\]

DESCRIPTION
===========
Upgrades splinter by importing data from the deprecated YAML state file format
to a database. This tool searches for data in the `circuits.yaml` and
`circuit-proposals.yaml` state definitions from the state directory. When the
upgrade is complete, the YAML state definitions will be renamed to
`circuits.yaml.old` and `circuit-proposals.yaml.old` respectively.

OPTIONS
=======
`-S`, `--state-dir` `STATE-DIR`
: Specifies the storage directory. (Defaults to `/var/lib/splinter`, unless
`SPLINTER_STATE_DIR` or `SPLINTER_HOME` is set.)

`-C`, `--connect` `DB-URL`
: Specifies the URL or connection string for the PostgreSQL or SQLite database
used for Splinter state. The default SQLite database will go in the directory,
`/var/lib/splinter`, unless `SPLINTER_STATE_DIR` or `SPLINTER_HOME` is set.

EXAMPLES
========
This example upgrades splinter by connecting to a PostgreSQL server
with the example hostname and port `splinter-db-alpha:5432`.

```
splinter upgrade -C postgres://admin:admin@splinter-db-alpha:5432/splinter
```

This example upgrades splinter connecting to the SQLite database
`./custom-sqlite.db` and using YAML state files from `./custom/dir`.

```
splinter upgrade -S ./custom/dir -C ./custom-sqlite.db
```

ENVIRONMENT
===========
The following environment variables affect the execution of the command.

**SPLINTER_STATE_DIR**

: Defines the default state directory for YAML state and SQLite. This is
overridden by the `--state-dir` flag

**SPLINTER_HOME**

: Defines the default splinter home directory, from which the state directory
is derived as `$SPLINTER_HOME/data`. This environment variable is not used if
either the `SPLINTER_STATE_DIR` environment variable or the `--state-dir` flag
is set.

SEE ALSO
========
| Splinter documentation: https://www.splinter.dev/docs/0.5/
