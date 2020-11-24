% SPLINTER-DATABASE-MIGRATE(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2020 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-database-migrate** — Updates the database for a new Splinter
release

SYNOPSIS
========

**splinter database migrate** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========

Splinter state can be stored in a PostgreSQL database or a SQLite database.

This command migrates the database from one Splinter release to the next.
If a new release adds new database tables or changes existing table formats,
run this command to update the database to the new format.

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

`-C` *CONNECTION-STRING*
: Specifies the connection string or URI for the database server.

EXAMPLES
========
This example migrates the database by connecting to a PostgreSQL server
with the example hostname and port `splinter-db-alpha:5432`.

```
splinter database migrate -C postgres://admin:admin@splinter-db-alpha:5432/splinter
```

SEE ALSO
========
| Splinter documentation: https://www.splinter.dev/docs/0.5/
