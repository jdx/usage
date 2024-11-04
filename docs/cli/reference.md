# `usage`
- **version**: 1.0.1

CLI for working with usage-based CLIs


- **Usage**: `usage [--usage-spec] [COMPLETIONS] <SUBCOMMAND>`

## Arguments

### `[COMPLETIONS]`

Outputs completions for the specified shell for completing the `usage` CLI itself

## Flags

### `--usage-spec`

Outputs a `usage.kdl` spec for this CLI itself

## `usage bash`

- **Usage**: `usage bash [-h] [--help] <SCRIPT> [ARGS]...`
- **Source code**: [`cli/src/cli/bash.rs`](https://github.com/jdx/usage/blob/main/cli/src/cli/bash.rs)

Executes a bash script

Typically, this will be called by a script's shebang

### Arguments

#### `<SCRIPT>`

#### `[ARGS]...`

arguments to pass to script

### Flags

#### `-h`

show help

#### `--help`

show help

## `usage complete-word`

- **Usage**: `usage complete-word [FLAGS] [WORDS]...`
- **Aliases**: `cw`
- **Source code**: [`cli/src/cli/complete-word.rs`](https://github.com/jdx/usage/blob/main/cli/src/cli/complete-word.rs)

### Arguments

#### `[WORDS]...`

user's input from the command line

### Flags

#### `--shell <SHELL>`

**Choices:**

- `bash`
- `fish`
- `zsh`

#### `-f --file <FILE>`

usage spec file or script with usage shebang

#### `-s --spec <SPEC>`

raw string spec input

#### `--cword <CWORD>`

current word index

## `usage exec`

- **Usage**: `usage exec <ARGS>…`
- **Aliases**: `x`
- **Source code**: [`cli/src/cli/exec.rs`](https://github.com/jdx/usage/blob/main/cli/src/cli/exec.rs)

### Arguments

#### `<COMMAND>`

command to execute after parsing usage spec

#### `<BIN>`

path to script to execute

#### `[ARGS]...`

arguments to pass to script

## `usage generate`

- **Usage**: `usage generate <SUBCOMMAND>`
- **Aliases**: `g`
- **Source code**: [`cli/src/cli/generate.rs`](https://github.com/jdx/usage/blob/main/cli/src/cli/generate.rs)

## `usage generate completion`

- **Usage**: `usage generate completion [--usage-cmd <USAGE_CMD>] [-f --file <FILE>] <SHELL> <BIN>`
- **Aliases**: `c`
- **Source code**: [`cli/src/cli/generate/completion.rs`](https://github.com/jdx/usage/blob/main/cli/src/cli/generate/completion.rs)

### Arguments

#### `<SHELL>`

**Choices:**

- `bash`
- `fish`
- `zsh`

#### `<BIN>`

The CLI which we're generates completions for

### Flags

#### `--usage-cmd <USAGE_CMD>`

A command which generates a usage spec e.g.: `mycli --usage` or `mycli completion usage` Defaults to "$bin --usage"

#### `-f --file <FILE>`

## `usage generate fig`

- **Usage**: `usage generate fig [FLAGS]`
- **Source code**: [`cli/src/cli/generate/fig.rs`](https://github.com/jdx/usage/blob/main/cli/src/cli/generate/fig.rs)

### Flags

#### `-f --file <FILE>`

A usage spec taken in as a file

#### `--spec <SPEC>`

raw string spec input

#### `--out-file <OUT_FILE>`

File on where to save the generated Fig spec

#### `--stdout`

Whether to output to stdout

## `usage generate markdown`

- **Usage**: `usage generate markdown <FLAGS>`
- **Aliases**: `md`
- **Source code**: [`cli/src/cli/generate/markdown.rs`](https://github.com/jdx/usage/blob/main/cli/src/cli/generate/markdown.rs)

### Flags

#### `-f --file <FILE>`

A usage spec taken in as a file

#### `-m --multi`

Render each subcommand as a separate markdown file

#### `--url-prefix <URL_PREFIX>`

Prefix to add to all URLs

#### `--html-encode`

Escape HTML in markdown

#### `--out-dir <OUT_DIR>`

Output markdown files to this directory

#### `--out-file <OUT_FILE>`
