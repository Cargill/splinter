% SPLINTER-DATABASE-MIGRATE(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2020 Cargill Incorporated

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
-->

NAME
====

**splinter-database-migrate** — Updates the Biome database for a new Splinter
release

SYNOPSIS
========

**splinter database migrate** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========

Biome is the Splinter module that manages users, credentials, and private keys,
using a PostgreSQL database to store this information.

This command migrates the Biome database from one Splinter release to the next.
If a new release adds new database tables or changes existing table formats,
run this command to update the Biome database to the new format.

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
This example migrates the Biome database by connecting to a PostgreSQL server
with the example hostname and port `splinter-db-alpha:5432`.

```
splinter database migrate -C postgres://admin:admin@splinter-db-alpha:5432/splinter
```

SEE ALSO
========
| Splinter documentation: https://github.com/Cargill/splinter-docs/blob/master/docs/index.md
