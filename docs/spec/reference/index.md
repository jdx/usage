# Top-level metadata

```sh
min_usage_version "1.0.0" # the minimum version of usage this CLI supports
                          # you want this at the top

name "My CLI"        # a friendly name for the CLI
bin "mycli"          # the name of the binary
version "1.0.0"      # the version of the CLI
author "nobody"      # the author of the CLI
license "MIT"        # SPDX license the CLI is released under

# help for -h
before_help "before about"
about "some help"
after_help "after about"

# help for --help
before_long_help "before about"
long_about "longer help"
after_long_help "after about"

# examples (shown in markdown and manpage docs)
example "mycli --help" header="Getting help" help="Display help information"
example "mycli --version"

# render a link to the source code in markdown docs
source_code_link_template "https://github.com/me/myproj/blob/main/src/cli/{{path}}.rs"

include "./my_overrides.usage.kdl" # include another spec, will be merged and override existing values
```

## Source Code Link Template

This is a tera template that can be used to customize the path for markdown documentation. For
example, in mise I use the following to convert filenames to snake case:

```sh
source_code_link_template r#"
{%- set path = path | replace(from='-', to='_') -%}
{%- if cmd.subcommands | length > 0 -%}
{%- set path = path | split(pat="/") | slice(end=1) | concat(with="mod.rs") | join(sep="/") -%}
{%- else -%}
{%- set path = path ~ ".rs" -%}
{%- endif -%}
https://github.com/jdx/mise/blob/main/src/cli/{{path}}"#
```

## Examples

Examples can be added at both the spec-level (top-level) and command-level to demonstrate CLI usage. Examples are displayed in generated markdown and manpage documentation.

### Spec-Level Examples

Top-level examples showcase general usage of your CLI:

```sh
name "demo"
bin "demo"

example "demo --help" header="Getting help" help="Display help information for the demo command"
example "demo --version" header="Check version" help="Show the installed version of demo"
```

### Command-Level Examples

Commands can also have their own examples (see [cmd reference](./cmd.md)):

```sh
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
