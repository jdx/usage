# Usage

Usage is a spec and CLI for defining CLI tools. Arguments, flags, environment variables, and config files
can all be defined in a Usage spec. It can be thought of like [OpenAPI (swagger)](https://www.openapis.org/)
for CLIs. Here are some potential reasons for defining your CLI with a Usage spec:

* Generate autocompletion scripts
* Generate markdown documentation
* Generate man pages
* Use an advanced arg parser in any language
* Scaffold one spec into different CLI frameworks—even different languages
* [coming soon] Host your CLI documentation on usage.sh

> [!WARNING]
>
> This README is aspirational and not all features are implemented yet.
> Until the project reaches 1.0, the spec and CLI are subject to change wildly.
>
> **rtx users**: Usage is safe to use for rtx despite being alpha.
> That said, you'll want to keep usage up to date as you update rtx.

## Example Usage Spec

Usage specs are written in [kdl](https://kdl.dev/) which is a newer document language that sort of combines
the best of XML and JSON. Here is a basic example CLI definition:

```sh
# optional metadata
name "My CLI"        # a friendly name for the CLI
bin "mycli"          # the name of the binary
about "some help"    # a short description of the CLI
version "1.0.0"      # the version of the CLI
author "nobody"      # the author of the CLI
license "MIT"        # license the CLI is released under

# a standard flag
flag "-f,--force"   help="Always do the thing"
flag "-v,--version" help="Print the CLI version"
flag "-h,--help"    help="Print the CLI help"

# a flag that takes a value
flag "-u,--user <user>" help="User to run as"

arg "<dir>"  help="The directory to use" # required positional argument
arg "[file]" help="The file to read"     # optional positional argument
```

And here is an example CLI with nested subcommands:

```sh
flag "-v,--verbose" "Enable verbose logging" global=true count=true
flag "-q,--quiet" "Enable quiet logging" global=true
flag "-u,--user <user>" help="User to run as"

cmd "update" help="Update the CLI"
cmd "config" help="Manage the CLI config" {
  # "set" is an alias for "add"
  cmd "add" "Add/set a config" {
    alias "set"
    arg "<key>" help="The key for the config"
    arg "<value>" help="The new config value"
    flag "-f,--force" help="Overwrite existing config"
  }
  cmd "remove" help="Remove a thing" {
    alias "rm"
    alias "delete" hide=true # hide alias from docs and completions
    arg "<name>" help="The name of the thing"
  }
  cmd "list" help="List all things"
}
cmd "version" help="Print the CLI version"
cmd "help" help="Print the CLI help"
```

Flags/args can be backed by config files, environment variables, or defaults:

```sh
config_file ".mycli.toml" findup=true
flag "-u,--user <user>" help="User to run as" env="MYCLI_USER" config="settings.user" default="admin"
```

The priority over which is used (CLI flag, env var, config file, default) is the order which they are defined,
so in this example it will be "CLI flag > env var > config file > default".

## Usage Scripts

Scripts can be used with the Usage CLI to display help, powerful arg parsing, and autocompletion in any language.
The Usage CLI can be used with "double-shebang" scripts which contain both the Usage definition and script in a
single file. Here is an example in bash:

```bash
#!/usr/bin/env usage
flag "-f,--force" help="Overwrite existing <file>"
flag "-u,--user <user>" help="User to run as"
arg "<file>" help="The file to write" default="file.txt"

#!/usr/bin/env bash
if [ "$usage_force" = "true" ]; then
  rm -f "$usage_file"
fi
if [ -n "$usage_user" ]; then
  echo "Hello, $usage_user" >> "$usage_file"
else
  echo "Hello, world" >> "$usage_file"
fi
```

Assuming this script was located at `./mycli`, it could be used like this:

```sh-session
$ ./mycli --help
Usage: mycli [flags] [args]
...
$ ./mycli -f --user=alice output.txt
$ cat output.txt
Hello, alice
```

## CLI Framework Developers

You could think of Usage like an LSP (Language Server Protocol) for CLIs.

Those building CLI frameworks can really benefit from Usage. Rather than building features like autocompletion
for every shell, just output a Usage definition and use the Usage CLI to generate autocompletion scripts for all
of the shells it supports.

## Generating Completion Scripts

Usage can generate completion scripts for any shell. Here is an example for bash:

```sh-session
$ usage g completion bash -f ./mycli.usage.kdl > ~/.bash_completions/mycli.bash
$ source ~/.bash_completions/mycli.bash
$ mycli --<TAB>
```

zsh:

```sh-session
$ usage g completion zsh -f ./mycli.usage.kdl > ~/.zsh_completions/_mycli
$ source ~/.zsh_completions/_mycli
$ mycli --<TAB>
```

fish:

```sh-session
$ usage g completion fish -f ./mycli.usage.kdl > ~/.config/fish/completions/mycli.fish
$ mycli --<TAB>
```

> [!IMPORTANT]
> 
> Usage CLI is a runtime dependency for the generated completion scripts. Your users
> will need to have `usage` installed in order for the completion scripts to work.

New shells should be easy to add because the logic around completions is mostly handled by the Usage CLI.
Typically, completion scripts will call usage like this to fetch completion choices (cword is the index of
the current word):

```sh-session
$ usage complete-word --file ./mycli.usage.kdl -- mycli cmd1 cmd2 --f
--force
--file
```

## Generating Manpages

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

## Generating Markdown Documentation

Usage CLI can generate markdown documentation from a Usage definition either into a single file, or a directory.

Single file (will be injected in the comment):

```sh-session
$ cat <<EOF > ./README.md
# My CLI
## Header
...
## CLI Commands
<!-- usage:start -->
## Footer
...
EOF
$ usage g markdown -f ./mycli.usage.kdl --inject README.md
$ cat README.md
# My CLI
## Header
## CLI Commands
<!-- usage:start -->
### `mycli config add KEY VALUE`
### `mycli config remove NAME`
<!-- usage:end   -->
## Footer
```

Multiple files:

```sh-session
$ usage g markdown -f ./mycli.usage.kdl --dir ./docs
$ tree ./docs
docs
├── config
│   ├── add.md
│   ├── list.md
│   └── remove.md
├── index.md
└── update.md
```

## Configuration

The default priority for configuration properties in usage is the following:

* CLI flag (e.g. `--user alice`)
* Environment variable (e.g. `MYCLI_USER=alice`)
* Config file (e.g. `~/.mycli.toml`)
* Default value

### Environment Variables

TODO

### Config Files

```kdl
config {
    // system
    file "/etc/mycli.toml"
    file "/etc/mycli.json"

    // global
    file "~/.config/mycli.toml"
    file "~/.config/mycli.json"

    // local
    file ".config/mycli.toml" findup=true
    file ".config/mycli.json" findup=true
    file ".mycli.dist.toml" findup=true
    file ".mycli.dist.json" findup=true
    file ".mycli.toml" findup=true
    file ".mycli.json" findup=true
    file ".myclirc" findup=true format="ini"

    // e.g.: .mycli.dev.toml, .mycli.prod.toml
    file ".mycli.$MYCLI_ENV.toml" findup=true

    default "user" "admin"
    default "work_dir" "/tmp"
    default "yes" false

    alias "user" "username"
}
```

#### Alias Config Keys

Config keys can be aliased to other keys. This is useful for backwards compatibility.

```kdl
config_file ".mycli.toml" findup=true
config_alias "user" "username"
```

## Compatibility

Usage is not designed to model every possible CLI. It's generally designed for CLIs that follow standard GNU-style
options. While it is not high priority, adding support for CLIs that differ from the standard may be allowed.
As an example, some CLIs may accept multiple options on a flag: `--flag option1 option2`. This is poor design
as it's unclear to the user if "option2" is another positional arg or not. What we will likely do for behaviors
like this is allow it, but show a warning that it is not recommended.
