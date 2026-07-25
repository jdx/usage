# Usage Specification

Usage is a spec and CLI for defining CLI tools. Arguments, flags, environment variables, and config
files can all be defined in a Usage spec. It can be thought of
like [OpenAPI (swagger)](https://www.openapis.org/)
for CLIs. Here are some potential reasons for defining your CLI with a Usage spec:

- Generate autocompletion scripts
- Generate markdown documentation
- Generate man pages
- Generate type-safe SDK client libraries for TypeScript, Python, and Rust
- Use an advanced arg parser in any language
- Scaffold one spec into different CLI frameworks—even different languages
- [coming soon] Host your CLI documentation on usage.sh

## Example Usage Spec

Usage specs are written in [kdl](https://kdl.dev/) which is a newer document language that sort of
combines
the best of XML and JSON. Here is a basic example CLI definition:

```kdl
// optional metadata
name "My CLI"        // a friendly name for the CLI
bin "mycli"          // the name of the binary
about "some help"    // a short description of the CLI
version "1.0.0"      // the version of the CLI
author "nobody"      // the author of the CLI
license "MIT"        // license the CLI is released under

// a standard flag
flag "-f --force"   help="Always do the thing"
flag "-v --version" help="Print the CLI version"
flag "-h --help"    help="Print the CLI help"

// a flag that takes a value
flag "-u --user <user>" help="User to run as"

arg "<dir>"  help="The directory to use" // required positional argument
arg "[file]" help="The file to read"     // optional positional argument
```

And here is an example CLI with nested subcommands:

```kdl
flag "-v --verbose" "Enable verbose logging" global=#true count=#true
flag "-q --quiet" "Enable quiet logging" global=#true
flag "-u --user <user>" help="User to run as"

cmd "update" help="Update the CLI"
cmd "config" help="Manage the CLI config" {
  // "set" is an alias for "add"
  cmd "add" "Add/set a config" {
    alias "set"
    arg "<key>" help="The key for the config"
    arg "<value>" help="The new config value"
    flag "-f --force" help="Overwrite existing config"
  }
  cmd "remove" help="Remove a thing" {
    alias "rm"
    alias "delete" hide=#true // hide alias from docs and completions
    arg "<name>" help="The name of the thing"
  }
  cmd "list" help="List all things"
}
cmd "version" help="Print the CLI version"
cmd "help" help="Print the CLI help"
```

Flags/args can be backed by config files, environment variables, or defaults:

```kdl
config_file ".mycli.toml" findup=#true
flag "-u --user <user>" help="User to run as" env="MYCLI_USER" config="settings.user" default="admin"
```

The priority over which is used (CLI flag, env var, config file, default) is the order which they
are defined,
so in this example it will be "CLI flag > env var > config file > default".

## Command effects

A command can declare what running it does to the world:

```kdl
cmd "ls"        effect="read"        help="List installed tools"
cmd "use"       effect="write"       help="Install a tool and add it to the config"
cmd "uninstall" effect="destructive" help="Remove a tool"
```

| Effect         | Meaning                                                                              |
| -------------- | ------------------------------------------------------------------------------------ |
| `read`         | Only inspects state. Running it twice is the same as running it once.                |
| `write`        | Creates or modifies state, but removes nothing the user cannot recreate.             |
| `destructive`  | May delete or irreversibly overwrite something. Deserves a confirmation prompt.      |

This is a coarse classification, not a permission model. It exists because
several consumers keep reinventing the same distinction:

- generated documentation and `--help` can mark destructive commands
- a wrapper script can require confirmation before running one
- an AI coding agent can be handed an allowlist of read-only commands instead of
  prompting on every invocation

`effect` is **not inherited by subcommands**. `git remote` and
`git remote remove` do different things, and quietly inheriting a parent's
effect would make the least safe reading of a spec the default one. A command
with no `effect` is unknown, not safe — consumers should treat the absence of a
value as "ask".

It can also be written as a child node, which is easier to generate:

```kdl
cmd "uninstall" {
  effect "destructive"
}
```

## Compatibility

Usage is not designed to model every possible CLI. It's generally designed for CLIs that follow
standard GNU-style
options. While it is not high priority, adding support for CLIs that differ from the standard may be
allowed.
As an example, some CLIs may accept multiple options on a flag: `--flag option1 option2`. This is
poor design
as it's unclear to the user if "option2" is another positional arg or not. What we will likely do
for behaviors
like this is allow it, but show a warning that it is not recommended.

## CLI Framework Developers

You could think of Usage like an LSP (Language Server Protocol) for CLIs.

Those building CLI frameworks can really benefit from Usage. Rather than building features like
autocompletion
for every shell, just output a Usage definition and use the Usage CLI to generate autocompletion
scripts for all
of the shells it supports.
