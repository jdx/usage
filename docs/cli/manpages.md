# Generating Manpages

Usage CLI can generate manpages from a Usage definition. Here is an example:

```sh-session
$ usage g manpage -f ./mycli.usage.kdl > /usr/share/man/man1/mycli.1
$ man mycli
mycli(1)                    General Commands Manual                   mycli(1)

NAME
       mycli - description

SYNOPSIS
       mycli [-h|--help] [-V|--version] <subcommands>

DESCRIPTION
       ...

OPTIONS
       -h, --help
              Print help (see a summary with '-h')
...
```
