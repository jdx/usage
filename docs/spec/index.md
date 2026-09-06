# Usage specification

A Usage spec describes a CLI's commands, arguments, flags, help, completion, and
configuration in [KDL](https://kdl.dev/). The [Usage CLI](/cli/) generates artifacts
from that definition, and the [Rust framework](/rust/) exports it from typed code.

New to Usage? [Write a spec and generate your first artifacts](/guide/getting-started).
For an existing application, [an integration](/spec/integrations) can export the
interface from your CLI framework.

## Read the syntax

Each line is a node. Values follow the node name, `key=value` properties add
metadata, and braces contain child nodes. Use `#true` and `#false` for booleans.
Angle brackets mark required values (`<file>`); square brackets mark optional
values (`[file]`). The next examples are separate specs.

## Example Usage Spec

KDL reads like a config file and nests like XML. A basic CLI:

```kdl
// optional metadata
name "My CLI"        // a friendly name for the CLI
bin "mycli"          // the name of the binary
about "Process project files" // a short description of the CLI
version "1.0.0"      // the version of the CLI
author "Example Team" // the author of the CLI
license "MIT"        // license the CLI is released under

// a standard flag
flag "-f --force"   help="Overwrite existing output"
flag "-v --version" help="Print the CLI version"
flag "-h --help"    help="Print the CLI help"

// a flag that takes a value
flag "-u --user <user>" help="User to run as"

arg "<dir>"  help="The directory to use" // required positional argument
arg "[file]" help="The file to read"     // optional positional argument
```

And here is an example CLI with nested subcommands:

```kdl
flag "-v --verbose" help="Enable verbose logging" global=#true count=#true
flag "-q --quiet" help="Enable quiet logging" global=#true
flag "-u --user <user>" help="User to run as"

cmd "update" help="Update the CLI"
cmd "config" help="Manage the CLI config" {
  // "set" is an alias for "add"
  cmd "add" help="Add/set a config" {
    alias "set"
    arg "<key>" help="The key for the config"
    arg "<value>" help="The new config value"
    flag "-f --force" help="Overwrite existing config"
  }
  cmd "remove" help="Remove a setting" {
    alias "rm"
    alias "delete" hide=#true // hide alias from docs and completions
    arg "<name>" help="Setting to remove"
  }
  cmd "list" help="List all settings"
}
cmd "version" help="Print the CLI version"
cmd "help" help="Print the CLI help"
```

Flags/args can be backed by config files, environment variables, or defaults:

```kdl
flag "-u --user <user>" help="User to run as"
config {
  file ".mycli.toml" findup=#true
  prop "settings.user" type="string" default="admin" {
    cli "--user"
    env "MYCLI_USER"
  }
}
```

The priority is always CLI flag > environment variable > config file > default. See the
[config reference](/spec/reference/config) for settings and files, and
[configuration resolution](/spec/resolution) for the complete precedence and merging rules.

## Command effects

A command can declare what running it does to the world:

```kdl
cmd "ls"        effect="read"        help="List installed tools"
cmd "use"       effect="write"       help="Install a tool and add it to the config"
cmd "uninstall" effect="destructive" help="Remove a tool"
```

| Effect        | Meaning                                                                         |
| ------------- | ------------------------------------------------------------------------------- |
| `read`        | Only inspects state. Running it twice is the same as running it once.           |
| `write`       | Creates or modifies state, but removes nothing the user cannot recreate.        |
| `destructive` | May delete or irreversibly overwrite something. Deserves a confirmation prompt. |

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

### Flags and arguments

Some commands are only dangerous depending on how they are invoked. A flag or an
argument can raise the effect when it is supplied:

```kdl
cmd "logs" effect="read" help="Show daemon logs" {
  flag "--clear" effect="destructive" help="Delete stored logs"
  flag "--follow"
}

cmd "settings" effect="read" {
  arg "[setting]"
  arg "[value]" effect="write"   // `settings foo` reads, `settings foo bar` writes
}
```

The effect of an invocation is the **maximum** of the command's effect and the
effect of every flag and argument actually supplied. `read` < `write` <
`destructive`, so the maximum is well defined, and `SpecCommand::effect_of`
computes it.

**Most flags should declare nothing.** The field is for the handful that change
what a command does to the world, not for annotating every option.

A flag or argument can only ever _raise_ the effect, never lower it. That makes
the rule safe to approximate: a consumer that has a spec but not a parsed
command line can take the maximum over the command and _all_ of its flags and
arguments — `SpecCommand::max_effect` — and still never under-report danger.

`--dry-run` is the tempting counterexample. Lowering is deliberately not
supported, because a bug in a dry-run path would then produce a spec that claims
a command is safe when it is not.

It can also be written as a child node, which is easier to generate:

```kdl
cmd "uninstall" {
  effect "destructive"
}
```

## Compatibility

Usage supports GNU-style long and short flags, attached or detached values,
bundled short flags, subcommands, and `--` to end flag parsing. Multi-value flags
are supported when explicitly declared, but they consume following values greedily;
use bounds or a separator when the boundary would otherwise be ambiguous.

The [argument grammar](/spec/argv) defines the exact rules, including unknown
flags and repeated values.

## For CLI Framework Authors

A framework that can emit a usage spec gets everything downstream of it without building any of
it. Completion for five shells, Markdown, man pages, and SDKs are each written once against the
spec rather than once per framework, so emitting the spec is the whole integration. Every
[integration](/spec/integrations) that exists is this shape. The same holds in reverse for a
framework built on the spec: the [Rust](/rust/) and [Go](/go/) frameworks answer a shared
conformance corpus, so a parser that passes it is known to agree with the reference
implementation.
