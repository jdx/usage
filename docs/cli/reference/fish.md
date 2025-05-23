# `usage fish`

- **Usage**: `usage fish [-h] [--help] <SCRIPT> [ARGS]…`
- **Source code**: [`cli/src/cli/fish.rs`](https://github.com/jdx/usage/blob/main/cli/src/cli/fish.rs)

Executes a shell script with the specified shell

Typically, this will be called by a script's shebang

If using `var=#true` on args/flags, they will be joined with spaces using `shell_words::join()`
to properly escape and quote values with spaces in them.

## Arguments

### `<SCRIPT>`

### `[ARGS]…`

arguments to pass to script

## Flags

### `-h`

show help

### `--help`

show help
