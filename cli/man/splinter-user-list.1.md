% SPLINTER-USER-LIST(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2022 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-user-list** — Displays the existing users for this Splinter node.

SYNOPSIS
========
**splinter user list** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========
This command lists all of the users the local node has. This command
displays abbreviated information pertaining to users in columns, with the
headers `ID`, `USERNAME`, and `TYPE`. This makes it possible to view all the
users registered with the local node, either through Biome or through one of
the various Splinter-supported OAuth providers. The `USERNAME` is either a
Biome user's `username` submitted at registration or the main `username` as
determined by an OAuth provider. The `TYPE` column displays the method used by
the user to register with Splinter, currently either `Biome` or `OAuth`. The
`ID` column maps to the user's internal ID, which is used while assigning
authorizations to a user.

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
`-F`, `--format` FORMAT
: Specifies the output format of the list. (default `human`). Possible values
  for formatting are `human` and `csv`.

`-k`, `--key` PRIVATE-KEY-FILE
: Specifies the private signing key (either a file path or the name of a
  .priv file in $HOME/.splinter/keys).

`-U`, `--url` URL
: Specifies the URL for the `splinterd` REST API. The URL is required unless
  `$SPLINTER_REST_API_URL` is set.

EXAMPLES
========
This command displays information about Splinter users with a default `human`
formatting, meaning the information is displayed in a table.

```
$ splinter user list \
  --url URL-of-splinterd-REST-API
ID                                    USERNAME    TYPE
f35aacc1-a9cd-4eda-b6d0-2efaddf0c8a4  oauth_user  OAuth
3no4hz9g-628s-m20x-b9a3-4ijodc402973  biome_user  Biome
```

ENVIRONMENT VARIABLES
=====================
**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| `splinter-role(1)`
| `splinter-role-create(1)`
| `splinter-permissions(1)`
|
| Splinter documentation: https://www.splinter.dev/docs/0.7/
