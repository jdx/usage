# Top-level metadata

```kdl
min_usage_version "1.0.0" // the minimum version of usage this CLI supports
                          // you want this at the top

name "My CLI"        // a friendly name for the CLI
bin "mycli"          // the name of the binary
version "1.0.0"      // the version of the CLI
long_version "1.0.0\ncommit abc123" // extended text for --version; -V uses version
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

// markdown used by generated markdown documentation
about_md "Longer **markdown** description"

// replace the generated synopsis
usage "mycli [OPTIONS] <COMMAND>"

// examples (shown in markdown and manpage docs)
example "mycli --help" header="Getting help" help="Display help information"
example "mycli --version"

// render a link to the source code in markdown docs
source_code_link_template "https://github.com/me/myproj/blob/main/src/cli/{{path}}.rs"

include file="./my_overrides.usage.kdl" // include another spec, will be merged and override existing values

// a reusable set of flags, pulled into a command with `use` (see ./flagset.md)
flagset "common" { flag "-v --verbose" }

// CLI-wide stdout formats and exit statuses (see ./output.md)
output "json" framing="json"
exit_code 0 "success"
exit_code 1 "error"
```

## Surface and availability metadata

Commands, arguments, and flags may carry descriptive contract metadata:

```kdl
surface "public"
available_if "supported-platform"

cmd "doctor" surface="internal" {
  available_if "debug-build" "admin-policy"
  flag "--json" surface="automation" available_if="json-feature"
}
```

`surface` is a project-defined audience or compatibility label such as `public`, `advanced`,
`automation`, or `internal`. `available_if` is an ordered list of project-defined conditions.
Neither changes parsing, help visibility, or validation: generators, documentation sites, API
catalogs, and migration tooling decide how to interpret them. Use `hide` for an entry that should
not be advertised and ordinary validation for a condition that must be enforced at runtime.

## Help variants

The plain fields (`about`, `before_help`, and `after_help`) are the short-help
forms. Their `long_` variants are used for long help. `about_md` supplies
Markdown directly to generated Markdown documentation; commands, flags, and
arguments have the corresponding `help_md`, and commands also accept
`before_help_md` and `after_help_md`. When no Markdown form is declared, the
generator falls back to long help and then short help.

`usage` replaces the generated synopsis at the root. A command's synopsis is
otherwise generated from its flags, arguments, and subcommands.

## Laying out help

`help_template` controls the order of the ten pre-rendered sections in every
help page:

```kdl
help_template "{{about}}\n\n{{usage}}\n\n{{flags}}\n\n{{args}}\n\n{{commands}}\n\n{{after_help}}"
```

The supported placeholders are `about`, `usage`, `commands`, `args`, `flags`,
`grouped_args`, `ungrouped_args`, `grouped_flags`, `ungrouped_flags`, and
`after_help`. `args` and `flags` contain their complete sections for backwards
compatibility; the grouped and ungrouped forms expose the same content in smaller
pieces when a port needs to interleave it. Ungrouped flags include inherited global
flags. A placeholder outside that closed vocabulary is rejected when the spec is
parsed. A template may omit a section or add literal text. Literal text and whole
sections may carry terminal styles with `{$style}…{/$}`:

```kdl
help_template "{$heading}My tool{/$}\n\n{{usage}}\n\n{$cyan}{{flags}}{/$}"
```

Styles may be combined with `+`. The semantic styles are `heading`, `option`, and
`metavar`; the physical styles are the eight ANSI colour names, their `bright-`
variants, `bold`, `dim`, `italic`, and `underline`. Plain and generated help removes
the tags. Tags in substituted descriptions are ordinary prose rather than template
markup. Double the dollar sign to write a delimiter literally: `{$$heading}` renders
`{$heading}`, and `{/$$}` renders `{/$}`.

## Root command policy

The root accepts the same command-policy nodes documented in the
[`cmd` reference](./cmd.md), except `restart_token`, which belongs to a named
`cmd`. These root-only settings are also available:

```kdl
default_subcommand "run"
disable_help #true
```

`default_subcommand` routes an unmatched first word to the named command. Known
subcommands still take precedence. `disable_help` disables the parser's built-in
recognition of `-h`, `--help`, `-?`, and `help`; use the narrower
`disable_help_flag` or `disable_help_subcommand` command policies when only one
entry point should be removed.

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

## Executable views

A `view` promotes a command path into a separately named executable surface. It is useful when
one binary is installed under several names but an applet is more than multicall dispatch: its
help, docs, and completions should begin at that command and may carry root globals.

```kdl
name "Aube"
bin "aube"
flag "-v --verbose" global=#true
flag "--config <FILE>" global=#true
view "aubr" root="run" globals=#true
view "aubx" name="Aube Execute" root="dlx" {
  global "--config"
}
cmd "run"
cmd "dlx"
```

The first string is the stable view identifier and defaults both `name` and `bin`. `root` is a
space-separated command path. `globals=#true` carries every root global; `global` children carry
only the named root globals. Generators accept a view explicitly (`usage g markdown --view aubr`)
or, for completions, select it when the requested binary matches the view's `bin`.

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

A Rust CLI says this on the root, as `#[usage(repository = "…")]` — an expression,
so `env!("CARGO_PKG_REPOSITORY")` keeps `Cargo.toml` the source of truth. clap has
no equivalent concept, so a spec generated by `clap_usage` cannot carry it; there,
declare it in an extra spec that is merged over the generated one.

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

A Rust CLI says this on the root too, as
`#[usage(source_code_link_template = r#"…"#)]`. A raw string keeps every leading
space, so write the template's lines unindented. As with `repository`, only a
`clap_usage`-generated spec needs an extra spec merged over it.

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
