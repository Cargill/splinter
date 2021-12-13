% SPLINTER-STATE-MIGRATE(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2021 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-state-migrate** — Move scabbard state to or from LMDB

SYNOPSIS
========
| **command** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========
Move scabbard state to or from LMDB, deleting from the input database. This
allows for reconfiguring Scabbard instances to switch between using
LMDB files for merkle state or using SQL based databases. The SQL URI provided
should be for the SQLite or PostgreSQL database that contains the rest of
Splinter state.

The command will prompt the user to make sure they wish to run the command as
once the merkle state has been successfully moved to the out target for a
service, the input data will be removed.

This command should not be run when the associated splinterd is currently
running.

FLAGS
=====
`-f`, `--force`
: Always attempt to move state, regardless of if there is existing data in the
  out database

`-h`, `--help`
: Prints help information

`-V`, `--version`
: Prints version information

`-q`, `--quiet`
: Do not display output

`-v`
: Increases verbosity (the opposite of -q). Specify multiple times for more
  output

`-y`, `--yes`
:  Do not prompt for confirmation

OPTIONS
=======

`--in` `IN_DATABASE`
: Database URI that currently contains the scabbard state. If state is in
  individual LMDB files, provide `lmdb`

`--out` `OUT_DATABASE`
: The database URI the scabbard state should end up in. If state should be put
  into individual LMDB files, provide `lmdb`

`--state-dir` `STATE-DIR`
: Specifies the storage directory. (Defaults to `/var/lib/splinter`, unless
  `SPLINTER_STATE_DIR` or `SPLINTER_HOME` is set.)


EXAMPLES
========

The following example moves the LMDB files into the SQLite database for the
splinter daemon:

```
$ splinter state migrate --in lmdb --out /var/lib/splinter/splinter_state.db
Attempting to migrate scabbard state from lmdb to /var/lib/splinter/splinter_state.db
Warning: This will purge the data from `--in` and only the current state root is stored, the rest are purged.
Are you sure you wish to migrate scabbard state? [y/N]
y
Migrating state data for GkV3z-S1YpG::b000
Scabbard state successfully migrated
```

To skip responding to the prompt, add `-y` or `--yes`:

```
$ splinter state migrate \
    --in lmdb \
    --out /var/lib/splinter/splinter_state.db \
    --yes
Attempting to migrate scabbard state from lmdb to /var/lib/splinter/splinter_state.db
Migrating state data for GkV3z-S1YpG::b000
Scabbard state successfully migrated
```

If the LMDB files are not in the configured state directory provide
`--state-dir`:

```
$ splinter state migrate \
    --in lmdb \
    --out /var/lib/splinter/splinter_state.db \
    --state-dir  home/node2/data/splinter_state.db \
    -y
Attempting to migrate scabbard state from lmdb to /var/lib/splinter/splinter_state.db
Migrating state data for GkV3z-S1YpG::b000
Scabbard state successfully migrated
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
| Splinter documentation: https://www.splinter.dev/docs/0.7/
