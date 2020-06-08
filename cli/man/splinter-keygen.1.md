% SPLINTER-KEYGEN(1) Cargill, Incorporated | Splinter Commands
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

**splinter-keygen** — Generates user and daemon keys for Splinter

SYNOPSIS
========

**splinter keygen** \[**FLAGS**\] \[**OPTIONS**\] \[*KEY-NAME*\]

DESCRIPTION
===========

This command generates secp256k1 public/private keys for Splinter.

If no option is specified, this command generates user keys that are stored in
the directory `$HOME/splinter/keys`. The `--system` flag generates keys for the
Splinter daemon (`splinterd`) that are stored in `/etc/splinter/keys`. The
`--key-dir` option generates keys in the specified directory.

The file names are determined by the user name, unless the `*KEY-NAME*` argument
is used.

FLAGS
=====

`-f`, `--force`
: Overwrites key files if they already exist.

`-h`, `--help`
: Prints help information

`-q`, `--quiet`
: Decreases verbosity (the opposite of -v). When specified, only errors or
  warnings will be output.

`--system`
: Generates system keys for `splinterd` in `/etc/splinter/keys`.

`-V`, `--version`
: Prints version information

`-v`
: Increases verbosity (the opposite of -q). Specify multiple times for more
  output.

OPTIONS
=======

`--key-dir DIRECTORY`
: Generates keys in the given `DIRECTORY`, creating the directory if it does not
  already exist.

ARGUMENTS
=========

`*KEY-NAME*`
: (Optional) Specifies the base name for the key files. By default, the user
  name is used.

EXAMPLES
========

This example generates user keys for a Splinter user who is logged in as
`paulbunyan`.

```
$ splinter keygen
writing file: "/Users/paulbunyan/splinter/keys/paulbunyan.priv"
writing file: "/Users/paulbunyan/splinter/keys/paulbunyan.pub"
```

This example generates keys for the user `babe` in the `/tmp` directory:

```
$ splinter keygen --key-dir /tmp babe
writing file: "/tmp/babe.priv"
writing file: "/tmp/babe.pub"
```

The next example generates system keys for the Splinter daemon, but specifies
`splinterd` as the base name for the files (instead of the user name).

```
$ splinter keygen --system splinterd
writing file: "/etc/splinter/keys/splinterd.priv"
writing file: "/etc/splinter/keys/splinterd.pub"
```

SEE ALSO
========

| Splinter documentation: https://github.com/Cargill/splinter-docs/blob/master/docs/index.md
