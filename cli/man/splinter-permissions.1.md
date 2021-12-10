% SPLINTER-PERMISSIONS(1) Cargill, Incorporated | Splinter Commands
<!--
  Copyright 2018-2020 Cargill Incorporated
  Licensed under Creative Commons Attribution 4.0 International License
  https://creativecommons.org/licenses/by/4.0/
-->

NAME
====

**splinter-permissions** — Lists REST API permissions for a Splinter node

SYNOPSIS
========
**splinter permissions** \[**FLAGS**\] \[**OPTIONS**\]

DESCRIPTION
===========
This command lists all permissions for the local Splinter node's REST API.

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
: Specifies the output format of the permissions. (default `human`). Possible
  values for formatting are `human`, `csv`, and `json`.

`-k`, `--key` PRIVATE-KEY-FILE
: Specifies the private signing key (either a file path or the name of a
  .priv file in $HOME/.splinter/keys).

`-U`, `--url` URL
: Specifies the URL for the `splinterd` REST API. The URL is required unless
  `$SPLINTER_REST_API_URL` is set.

EXAMPLES
========
The following command displays REST API permissions in a human-readable table
(the output is abbreviated for readability):

```
$ splinter permissions \
  --key /path/to/key.priv \
  --url http://example.com:8080
ID                              NAME                   DESCRIPTION
authorization.maintenance.read  Maintenance mode read  Allows the client to check maintenance mode status
authorization.maintenance.write Maintenance mode write Allows the client to enable/disable maintenance mode
authorization.permissions.read  Permissions read       Allows the client to read REST API permissions
authorization.rbac.read         RBAC read              Allows the client to read roles, identities, and role assignments
...
```

The following command displays REST API permissions as CSV (output abbreviated):

```
$ splinter permissions \
  --format csv
  --key /path/to/key.priv \
  --url http://example.com:8080
ID,NAME,DESCRIPTION
authorization.maintenance.read,Maintenance mode read,Allows the client to check maintenance mode status
authorization.maintenance.write,Maintenance mode write,Allows the client to enable/disable maintenance mode
authorization.permissions.read,Permissions read,Allows the client to read REST API permissions
authorization.rbac.read,RBAC read,Allows the client to read roles, identities, and role assignments
...
```

The following command displays REST API permissions as JSON (output
abbreviated):

```
$ splinter permissions \
  --format json
  --key /path/to/key.priv \
  --url http://example.com:8080
[
 [
   "ID",
   "NAME",
   "DESCRIPTION"
 ],
 [
   "authorization.maintenance.read",
   "Maintenance mode read",
   "Allows the client to check maintenance mode status"
 ],
 [
   "authorization.maintenance.write",
   "Maintenance mode write",
   "Allows the client to enable/disable maintenance mode"
 ],
 [
   "authorization.permissions.read",
   "Permissions read",
   "Allows the client to read REST API permissions"
 ],
 [
   "authorization.rbac.read",
   "RBAC read",
   "Allows the client to read roles, identities, and role assignments"
 ],
  ...
]
```

ENVIRONMENT VARIABLES
=====================
**SPLINTER_REST_API_URL**
: URL for the `splinterd` REST API. (See `-U`, `--url`.)

SEE ALSO
========
| Splinter documentation: https://www.splinter.dev/docs/0.6/
