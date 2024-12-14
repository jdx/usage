# `usage generate completion`

- **Usage**: `usage generate completion [FLAGS] <SHELL> <BIN>`
- **Aliases**: `c`
- **Source code**: [`cli/src/cli/generate/completion.rs`](https://github.com/jdx/usage/blob/main/cli/src/cli/generate/completion.rs)

## Arguments

### `<SHELL>`

**Choices:**

- `bash`
- `fish`
- `zsh`

### `<BIN>`

The CLI which we're generates completions for

## Flags

### `--cache-key <CACHE_KEY>`

A cache key to use for storing the results of calling the CLI with --usage-cmd

### `-f --file <FILE>`

A .usage.kdl spec file to use for generating completions

### `--usage-bin <USAGE_BIN>`

Override the bin used for calling back to usage-cli

You may need to set this if you have a different bin named "usage"

### `--usage-cmd <USAGE_CMD>`

A command which generates a usage spec e.g.: `mycli --usage` or `mycli completion usage` Defaults to "$bin --usage"

### `--include-bash-completion-lib`

Include https://github.com/scop/bash-completion

This is required for usage completions to work in bash, but the user may already provide it
