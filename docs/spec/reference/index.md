# Top-level metadata

```kdl
min_usage_version "1.0.0" // the minimum version of usage this CLI supports
                          // you want this at the top

name "My CLI"        // a friendly name for the CLI
bin "mycli"          // the name of the binary
version "1.0.0"      // the version of the CLI
author "nobody"      // the author of the CLI
license "MIT"        // SPDX license the CLI is released under
repository "https://github.com/me/myproj" // where the source lives

// help for -h
before_help "before about"
about "some help"
after_help "after about"

// help for --help
before_long_help "before about"
long_about "longer help"
after_long_help "after about"

// examples (shown in markdown and manpage docs)
example "mycli --help" header="Getting help" help="Display help information"
example "mycli --version"

// render a link to the source code in markdown docs
source_code_link_template "https://github.com/me/myproj/blob/main/src/cli/{{path}}.rs"

include file="./my_overrides.usage.kdl" // include another spec, will be merged and override existing values
```

## Multicall

`multicall #true` is clap's busybox-style applets: argv[0]'s basename selects a
subcommand. The dispatcher names (`name` and `bin`) are skipped, so
`busybox ls` still runs the `ls` applet. A symlink `ls -> busybox` does too,
because the basename is `ls`. Path components and a trailing `.exe` are stripped.

```kdl
name "busybox"
bin "busybox"
multicall #true
cmd "ls"
cmd "cat"
```

clap exposes this as `Command::multicall` / `#[command(multicall = true)]`, and
the bridge reads `is_multicall_set`.

## Repository

The URL of the CLI's source repository:

```kdl
repository "https://github.com/jdx/mise"
```

This is the plain URL, not a template — the same value a `Cargo.toml`,
`package.json` or `pyproject.toml` carries. It is available to documentation
templates as `repository`, and it gives anything reading a spec out of context —
a docs site, a registry, an agent handed a `.usage.kdl` file — a way to get back
to the project.

It is deliberately separate from `source_code_link_template` below. That one is a
per-command deep link with a `{{path}}` placeholder, so recovering a repository
URL from it means pattern-matching one forge's URL layout, and it is absent from
most specs. Set both if you want both; neither implies the other.

clap has no equivalent concept, so `clap_usage` cannot generate this. Declare it
in an extra spec that is merged over the generated one — the same place
`source_code_link_template` is set.

## Source Code Link Template

This is a tera template that can be used to customize the path for markdown documentation. For
example, in mise I use the following to convert filenames to snake case:

```kdl
source_code_link_template #"""
{%- set path = path | replace(from='-', to='_') -%}
{%- if cmd.subcommands | length > 0 -%}
{%- set path = path | split(pat="/") | slice(end=1) | concat(with="mod.rs") | join(sep="/") -%}
{%- else -%}
{%- set path = path ~ ".rs" -%}
{%- endif -%}
https://github.com/jdx/mise/blob/main/src/cli/{{path}}
"""#
```

## Examples

Examples can be added at both the spec-level (top-level) and command-level to demonstrate CLI usage. Examples are displayed in generated markdown and manpage documentation.

### Spec-Level Examples

Top-level examples showcase general usage of your CLI:

```kdl
name "demo"
bin "demo"

example "demo --help" header="Getting help" help="Display help information for the demo command"
example "demo --version" header="Check version" help="Show the installed version of demo"
```

### Command-Level Examples

Commands can also have their own examples (see [cmd reference](./cmd.md)):

```kdl
cmd "deploy" {
  flag "-e --environment <env>" help="Target environment"

  example "demo deploy -e prod" header="Basic deployment" help="Deploy to production environment"
  example "demo deploy -e staging --force" header="Force deployment"
}
```

### Example Properties

Each example supports the following properties:

- **code** (required): The command to demonstrate (first positional argument)
- **header** (optional): A title for the example
- **help** (optional): Description of what the example does
- **lang** (optional): Programming language for syntax highlighting in markdown (defaults to empty)
