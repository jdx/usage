# Generating Manpages

Usage CLI can generate Unix man pages from a Usage definition in the standard roff format.

## Basic Usage

Generate a manpage and output to stdout:

```bash
$ usage generate manpage -f ./mycli.usage.kdl
```

Save to a file:

```bash
$ usage generate manpage -f ./mycli.usage.kdl -o mycli.1
```

Install to the system man directory:

```bash
$ usage generate manpage -f ./mycli.usage.kdl | sudo tee /usr/share/man/man1/mycli.1
$ sudo mandb  # Update the man database
$ man mycli
```

## Manual Sections

You can specify the manual section with the `--section` flag (default is 1):

```bash
$ usage generate manpage -f ./myconfig.usage.kdl --section 5 -o myconfig.5
```

Common manual sections:

- **1**: User commands (default)
- **5**: File formats and conventions
- **7**: Miscellaneous (including macro packages and conventions)
- **8**: System administration commands and daemons

## Output Format

The generated man page follows the standard Unix man page format:

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

## Aliases

The command has a short alias for convenience:

```bash
$ usage g man -f ./mycli.usage.kdl
```

## Viewing Generated Pages

To preview the generated man page without installing it:

```bash
$ usage g man -f ./mycli.usage.kdl | man -l -
```

Or save and view:

```bash
$ usage g man -f ./mycli.usage.kdl -o mycli.1
$ man ./mycli.1
```
