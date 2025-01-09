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
example "mycli --help"

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
