# Top-level metadata

::: warning
This spec is still a work in progress and is subject to change until 1.0
:::

```sh
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
