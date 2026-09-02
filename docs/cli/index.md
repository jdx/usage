# CLI

`usage` is the command-line utility around the [usage spec](/spec/). A spec describes a CLI's
commands, flags, and arguments once, in KDL. This tool turns that one description into the
things a CLI ships with, and runs scripts that carry a spec in their own comments.

| To get                                                      | Run                          | Guide                                            |
| ----------------------------------------------------------- | ---------------------------- | ------------------------------------------------ |
| Tab completion for bash, zsh, fish, PowerShell, and Nushell | `usage generate completion`  | [Completions](/cli/completions)                  |
| Markdown reference pages                                    | `usage generate markdown`    | [Markdown](/cli/markdown)                        |
| Man pages                                                   | `usage generate manpage`     | [Manpages](/cli/manpages)                        |
| Typed TypeScript and Python clients                         | `usage generate sdk`         | [SDK generation](/cli/sdk)                       |
| Parse tables for a Go CLI                                   | `usage generate go`          | [Go framework](/go/)                             |
| A JSON Schema for the CLI's config file                     | `usage generate json-schema` | [reference](/cli/reference/generate/json-schema) |
| A shell script with parsed arguments and `--help`           | `usage bash`, `usage exec`   | [Scripts](/cli/scripts)                          |
| What changed between two versions of an interface           | `usage diff`                 | [Comparing specs](/cli/diff)                     |
| What a command line binds to, and why                       | `usage explain`              | [reference](/cli/reference/explain)              |
| Mistakes a spec can have and still parse                    | `usage lint`                 | [reference](/cli/reference/lint)                 |
| A CLI described to an AI agent, with each command's effect  | `usage mcp`                  | [reference](/cli/reference/mcp)                  |

Every command that reads a spec takes it the same three ways: a `.usage.kdl` file with `-f`, a
script whose `#USAGE` comments declare it, or `-` for stdin. A binary built with the
[Rust framework](/rust/) prints its own spec, so documenting it needs no file at all:

```sh
mycli __usage_spec__ | usage generate markdown -mf - --out-dir docs
```

The [reference](/cli/reference/) lists every command and flag. It is generated from `usage`'s
own spec by the command it documents.

## Installation

### [mise-en-place](https://mise.jdx.dev)

```sh
mise use -g usage
```

### Cargo

```sh
cargo install usage-cli
```

[`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall) fetches a prebuilt binary
instead of compiling one:

```sh
cargo binstall usage-cli
```

### Homebrew

```sh
brew install usage
```

### Arch Linux

The package is in [Extra](https://archlinux.org/packages/extra/x86_64/usage/):

```sh
pacman -S usage
```
