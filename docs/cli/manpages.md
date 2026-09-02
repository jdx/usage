# Generating Manpages

`usage generate manpage` renders a spec as roff, the format `man` reads. `g` and `man` are the
short aliases, and without `-o` the page goes to stdout:

```bash
usage g man -f ./mycli.usage.kdl -o mycli.1
```

Piping it straight into `man` previews the page without installing anything:

```bash
usage g man -f ./mycli.usage.kdl | man -l -
```

## Installing

A page lives in the directory for its section, and `mandb` has to be told it is there:

```bash
usage g man -f ./mycli.usage.kdl | sudo tee /usr/share/man/man1/mycli.1 > /dev/null
sudo mandb
man mycli
```

## Sections

The page is section 1, user commands, unless `--section` says otherwise:

```bash
usage g man -f ./myconfig.usage.kdl --section 5 -o myconfig.5
```

| Section | Contents                                                           |
| ------- | ------------------------------------------------------------------ |
| 1       | User commands (default)                                            |
| 5       | File formats and conventions, such as a page about the config file |
| 7       | Miscellaneous: overviews and conventions                           |
| 8       | System administration commands and daemons                         |

## What the page contains

The spec's `about` and long help become NAME and DESCRIPTION, its flags and subcommands become
OPTIONS and COMMANDS, and any `example` nodes become EXAMPLES:

```
mycli(1)                    General Commands Manual                   mycli(1)

NAME
       mycli - description of your CLI tool

SYNOPSIS
       mycli [OPTIONS] <COMMAND>

DESCRIPTION
       Detailed description of your CLI tool...

OPTIONS
       -h, --help
              Print help information

       -v, --verbose
              Enable verbose output

COMMANDS
       install
              Install a plugin

       list
              List installed plugins

EXAMPLES
       Install a plugin:

           mycli install my-plugin

AUTHOR
       Your Name <your.email@example.com>
```
