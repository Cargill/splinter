% SPLINTER-KEYGEN(1) Cargill, Incorporated | Splinter Commands

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
Splinter daemon (`splinterd`) that are stored in `/etc/splinter/keys`.

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
