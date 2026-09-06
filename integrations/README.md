# Framework integrations

Integrations that generate usage specs from CLI framework definitions, so help, completion, and documentation can share the application's declaration.

For the full list of available and planned integrations, see the [Integrations documentation](https://usage.jdx.dev/spec/integrations).

## How integrations work

An integration extracts the CLI definition (commands, flags, args, completions) from a framework's internal representation and outputs a [usage spec](https://usage.jdx.dev/spec/) in KDL format. This spec can then drive:

- **Shell completions** for bash, zsh, fish, PowerShell, and Nushell
- **Markdown documentation**
- **Man pages**
- **`--help` output**

See [`clap_usage`](../clap_usage/) for a reference implementation.

## Available in this repository

| Framework   | Package                        | Guide                                                              |
| ----------- | ------------------------------ | ------------------------------------------------------------------ |
| clap (Rust) | [`clap_usage`](../clap_usage/) | [clap integration](https://usage.jdx.dev/spec/integrations/clap)   |
| Cobra (Go)  | [`integrations/cobra`](cobra/) | [Cobra integration](https://usage.jdx.dev/spec/integrations/cobra) |

Exporting a spec does not replace the source framework's parser. Check the
integration's supported feature mapping before relying on generated artifacts,
and include a way for users to print the installed executable's current spec.
